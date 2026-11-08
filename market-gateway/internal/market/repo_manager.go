package market

import (
	"errors"
	"fmt"
	"sort"

	"go.uber.org/zap"
)

// repoState is the repo part of the Manager, guarded by Manager.mu so that
// a snapshot sees curves, specials and prices from the same instant.
type repoState struct {
	asof     Date
	gcCurves map[string]RepoCurve
	specials map[string]SpecialSpread
	fixings  map[string]FixingSeries
	bonds    map[string]BondStatic
	prices   map[string]BondPrice
	haircuts map[string]Haircut
}

func newRepoState() repoState {
	return repoState{
		gcCurves: map[string]RepoCurve{},
		specials: map[string]SpecialSpread{},
		fixings:  map[string]FixingSeries{},
		bonds:    map[string]BondStatic{},
		prices:   map[string]BondPrice{},
		haircuts: map[string]Haircut{},
	}
}

// snapshot deep-copies the state. Callers hold at least a read lock.
func (r *repoState) snapshot() RepoSnapshot {
	s := RepoSnapshot{
		SchemaVersion: RepoSchemaVersion,
		AsOf:          r.asof,
		GCCurves:      make(map[string]RepoCurve, len(r.gcCurves)),
		Specials:      make(map[string]SpecialSpread, len(r.specials)),
		Fixings:       make(map[string]FixingSeries, len(r.fixings)),
		Bonds:         make(map[string]BondStatic, len(r.bonds)),
		Prices:        make(map[string]BondPrice, len(r.prices)),
		Haircuts:      make(map[string]Haircut, len(r.haircuts)),
	}
	for k, c := range r.gcCurves {
		c.Pillars = append([]RepoPillar(nil), c.Pillars...)
		s.GCCurves[k] = c
	}
	for k, f := range r.fixings {
		f.Fixings = append([]Fixing(nil), f.Fixings...)
		s.Fixings[k] = f
	}
	for k, v := range r.specials {
		s.Specials[k] = v
	}
	for k, v := range r.bonds {
		s.Bonds[k] = v
	}
	for k, v := range r.prices {
		s.Prices[k] = v
	}
	for k, v := range r.haircuts {
		s.Haircuts[k] = v
	}
	return s
}

func stateFromSnapshot(s RepoSnapshot) repoState {
	// snapshot() of a state built on the same maps is a deep copy.
	tmp := repoState{
		asof: s.AsOf, gcCurves: s.GCCurves, specials: s.Specials, fixings: s.Fixings,
		bonds: s.Bonds, prices: s.Prices, haircuts: s.Haircuts,
	}
	c := tmp.snapshot()
	return repoState{
		asof: c.AsOf, gcCurves: c.GCCurves, specials: c.Specials, fixings: c.Fixings,
		bonds: c.Bonds, prices: c.Prices, haircuts: c.Haircuts,
	}
}

var errNoAsOf = errors.New("repo as-of date not set (call SetRepoAsOf first)")

// SetRepoAsOf sets the business date of the repo market. Pillar days of
// existing GC curves are recomputed from their tenors. The change is
// rejected if existing data is inconsistent with the new date (matured
// bonds, fixings after the date).
func (m *Manager) SetRepoAsOf(asof Date) error {
	m.mu.Lock()
	defer m.mu.Unlock()

	s := m.repo.snapshot()
	s.AsOf = asof
	for ccy, c := range s.GCCurves {
		for i := range c.Pillars {
			tn, err := ParseTenor(c.Pillars[i].Tenor)
			if err != nil {
				return err
			}
			c.Pillars[i].Days = tn.Days(asof.Time)
		}
		s.GCCurves[ccy] = c
	}
	if err := s.Validate(); err != nil {
		return fmt.Errorf("cannot move repo as-of to %s: %w", asof, err)
	}
	m.repo = stateFromSnapshot(s)
	m.logger.Info("set repo as-of date", zap.Stringer("asof", asof))
	return nil
}

// TenorQuote is a quoted GC repo rate for a tenor.
type TenorQuote struct {
	Tenor string
	Rate  float64
}

// UpdateGCCurve replaces the GC repo curve of a currency. Quotes may come
// in any order; pillar days are computed from the repo as-of date.
func (m *Manager) UpdateGCCurve(currency string, dc DayCount, quotes []TenorQuote) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.repo.asof.IsZero() {
		return errNoAsOf
	}
	curve := RepoCurve{DayCount: dc, Pillars: make([]RepoPillar, 0, len(quotes))}
	for _, q := range quotes {
		tn, err := ParseTenor(q.Tenor)
		if err != nil {
			return fmt.Errorf("gc_curves[%s]: %w", currency, err)
		}
		curve.Pillars = append(curve.Pillars, RepoPillar{Tenor: q.Tenor, Days: tn.Days(m.repo.asof.Time), Rate: q.Rate})
	}
	sort.SliceStable(curve.Pillars, func(i, j int) bool { return curve.Pillars[i].Days < curve.Pillars[j].Days })
	if err := ValidateCurve(m.repo.asof, currency, curve); err != nil {
		return err
	}
	m.repo.gcCurves[currency] = curve
	m.logger.Info("updated GC repo curve", zap.String("currency", currency), zap.Int("pillars", len(curve.Pillars)))
	return nil
}

// UpdateBond sets the static data of a collateral bond.
func (m *Manager) UpdateBond(isin string, b BondStatic) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.repo.asof.IsZero() {
		return errNoAsOf
	}
	if err := ValidateBond(m.repo.asof, isin, b); err != nil {
		return err
	}
	if old, ok := m.repo.bonds[isin]; ok && old.Currency != b.Currency {
		return fmt.Errorf("bonds[%s]: currency change %s -> %s not allowed", isin, old.Currency, b.Currency)
	}
	m.repo.bonds[isin] = b
	m.logger.Info("updated bond static", zap.String("isin", isin))
	return nil
}

func (m *Manager) requireBond(section, isin string) (BondStatic, error) {
	b, ok := m.repo.bonds[isin]
	if !ok {
		return BondStatic{}, fmt.Errorf("%s[%s]: no bond static for this ISIN", section, isin)
	}
	return b, nil
}

// UpdateSpecial sets how far an ISIN trades below GC. The bond and a GC
// curve in its currency must already exist.
func (m *Manager) UpdateSpecial(isin string, spread float64) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	b, err := m.requireBond("specials", isin)
	if err != nil {
		return err
	}
	if _, ok := m.repo.gcCurves[b.Currency]; !ok {
		return fmt.Errorf("specials[%s]: no GC curve for currency %s", isin, b.Currency)
	}
	if err := checkRate(fmt.Sprintf("specials[%s] spread", isin), spread); err != nil {
		return err
	}
	m.repo.specials[isin] = SpecialSpread{Spread: spread}
	m.logger.Info("updated special spread", zap.String("isin", isin), zap.Float64("spread", spread))
	return nil
}

// UpdateBondPrice sets the clean price (per face) of a bond.
func (m *Manager) UpdateBondPrice(isin string, clean float64) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if _, err := m.requireBond("prices", isin); err != nil {
		return err
	}
	if !finite(clean) || clean <= 0 {
		return fmt.Errorf("prices[%s]: clean price %v must be positive", isin, clean)
	}
	m.repo.prices[isin] = BondPrice{Clean: clean}
	m.logger.Debug("updated bond price", zap.String("isin", isin), zap.Float64("clean", clean))
	return nil
}

// UpdateHaircut sets the margining of a bond.
func (m *Manager) UpdateHaircut(isin string, h Haircut) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if _, err := m.requireBond("haircuts", isin); err != nil {
		return err
	}
	if err := h.Validate(); err != nil {
		return fmt.Errorf("haircuts[%s]: %w", isin, err)
	}
	m.repo.haircuts[isin] = h
	m.logger.Info("updated haircut", zap.String("isin", isin), zap.String("kind", string(h.Kind)), zap.Float64("value", h.Value))
	return nil
}

// AddFixing records an overnight fixing, inserting it in date order or
// replacing an existing fixing on the same date. The series is created on
// first use; later fixings must use the same currency and day count.
func (m *Manager) AddFixing(index, currency string, dc DayCount, f Fixing) error {
	m.mu.Lock()
	defer m.mu.Unlock()
	if m.repo.asof.IsZero() {
		return errNoAsOf
	}
	s, ok := m.repo.fixings[index]
	if !ok {
		s = FixingSeries{Currency: currency, DayCount: dc}
	} else if s.Currency != currency || s.DayCount != dc {
		return fmt.Errorf("fixings[%s]: series is %s %s, got %s %s", index, s.Currency, s.DayCount, currency, dc)
	}
	f.Date = NewDate(f.Date.Time)
	fixings := append([]Fixing(nil), s.Fixings...)
	i := sort.Search(len(fixings), func(i int) bool { return !fixings[i].Date.Before(f.Date.Time) })
	if i < len(fixings) && fixings[i].Date.Equal(f.Date.Time) {
		fixings[i] = f
	} else {
		fixings = append(fixings, Fixing{})
		copy(fixings[i+1:], fixings[i:])
		fixings[i] = f
	}
	s.Fixings = fixings
	if err := ValidateFixings(m.repo.asof, index, s); err != nil {
		return err
	}
	m.repo.fixings[index] = s
	return nil
}

// GetRepoSnapshot returns a validated deep copy of the repo market.
func (m *Manager) GetRepoSnapshot() (RepoSnapshot, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()
	if m.repo.asof.IsZero() {
		return RepoSnapshot{}, errNoAsOf
	}
	s := m.repo.snapshot()
	if err := s.Validate(); err != nil {
		return RepoSnapshot{}, err
	}
	return s, nil
}

// LoadRepoSnapshot validates and atomically replaces the repo market.
func (m *Manager) LoadRepoSnapshot(s RepoSnapshot) error {
	if err := s.Validate(); err != nil {
		return fmt.Errorf("invalid repo snapshot: %w", err)
	}
	st := stateFromSnapshot(s)
	m.mu.Lock()
	defer m.mu.Unlock()
	m.repo = st
	m.logger.Info("loaded repo snapshot",
		zap.Stringer("asof", s.AsOf),
		zap.Int("gc_curves", len(s.GCCurves)),
		zap.Int("bonds", len(s.Bonds)),
		zap.Int("fixing_series", len(s.Fixings)),
	)
	return nil
}

// RepoStats counts the repo market data held by the manager.
func (m *Manager) RepoStats() map[string]int {
	m.mu.RLock()
	defer m.mu.RUnlock()
	n := 0
	for _, f := range m.repo.fixings {
		n += len(f.Fixings)
	}
	return map[string]int{
		"gc_curves": len(m.repo.gcCurves),
		"specials":  len(m.repo.specials),
		"bonds":     len(m.repo.bonds),
		"prices":    len(m.repo.prices),
		"haircuts":  len(m.repo.haircuts),
		"fixings":   n,
	}
}
