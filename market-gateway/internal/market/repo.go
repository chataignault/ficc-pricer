package market

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"sort"
	"time"
)

// RepoSchemaVersion is the version of the repo market snapshot exchanged
// with the Rust repo-analytics crate. Bump it on any change to the JSON
// layout: the Rust loader rejects unknown versions and unknown fields.
const RepoSchemaVersion = 1

const dateLayout = "2006-01-02"

// Date is a calendar (business) date, serialised as YYYY-MM-DD.
type Date struct{ time.Time }

// NewDate truncates t to its calendar date in t's location.
func NewDate(t time.Time) Date {
	return Date{time.Date(t.Year(), t.Month(), t.Day(), 0, 0, 0, 0, time.UTC)}
}

// ParseDate parses YYYY-MM-DD.
func ParseDate(s string) (Date, error) {
	t, err := time.Parse(dateLayout, s)
	if err != nil {
		return Date{}, fmt.Errorf("invalid date %q (want YYYY-MM-DD): %w", s, err)
	}
	return Date{t}, nil
}

func (d Date) String() string { return d.Format(dateLayout) }

func (d Date) MarshalJSON() ([]byte, error) { return json.Marshal(d.String()) }

func (d *Date) UnmarshalJSON(b []byte) error {
	var s string
	if err := json.Unmarshal(b, &s); err != nil {
		return err
	}
	p, err := ParseDate(s)
	if err != nil {
		return err
	}
	*d = p
	return nil
}

// DayCount is a money-market or bond day count convention. The spellings
// are part of the schema and match repo-analytics `DayCount::from_str`.
type DayCount string

const (
	Act360    DayCount = "ACT/360"
	Act365F   DayCount = "ACT/365F"
	Thirty360 DayCount = "30/360"
)

func (dc DayCount) Validate() error {
	switch dc {
	case Act360, Act365F, Thirty360:
		return nil
	}
	return fmt.Errorf("unknown day count %q (want ACT/360, ACT/365F or 30/360)", string(dc))
}

// RepoPillar is one quoted point of a GC repo curve. Days is derived from
// Tenor and the snapshot as-of date; both are sent so the consumer can
// check that it uses the same tenor arithmetic.
type RepoPillar struct {
	Tenor string  `json:"tenor"`
	Days  int     `json:"days"`
	Rate  float64 `json:"rate"` // simple annual rate, decimal (0.0365 = 3.65%)
}

// RepoCurve is a general-collateral repo curve for one currency.
type RepoCurve struct {
	DayCount DayCount     `json:"day_count"`
	Pillars  []RepoPillar `json:"pillars"`
}

// SpecialSpread is how far an ISIN's repo rate trades below GC in its
// currency (decimal, positive = special).
type SpecialSpread struct {
	Spread float64 `json:"spread"`
}

// Fixing is one overnight index fixing.
type Fixing struct {
	Date Date    `json:"date"`
	Rate float64 `json:"rate"`
}

// FixingSeries is the history of an overnight index (SOFR, €STR, SONIA...).
type FixingSeries struct {
	Currency string   `json:"currency"`
	DayCount DayCount `json:"day_count"`
	Fixings  []Fixing `json:"fixings"` // strictly increasing dates
}

// BondStatic is the static data of a fixed-coupon bond used as collateral.
// Coupon dates are generated backwards from maturity by the consumer.
type BondStatic struct {
	Currency   string   `json:"currency"`
	CouponRate float64  `json:"coupon_rate"` // decimal (0.0425 = 4.25%)
	Frequency  int      `json:"frequency"`   // coupons per year: 1, 2, 4 or 12
	Maturity   Date     `json:"maturity"`
	Face       float64  `json:"face"` // price basis, usually 100
	DayCount   DayCount `json:"day_count"`
}

// BondPrice is a clean price per BondStatic.Face. Accrued interest is
// computed by the consumer: never send dirty prices here.
type BondPrice struct {
	Clean float64 `json:"clean"`
}

// HaircutKind selects how collateral is margined.
type HaircutKind string

const (
	// KindHaircut: cash = MV × (1 − value), e.g. 0.02.
	KindHaircut HaircutKind = "haircut"
	// KindInitialMargin: cash = MV / value, e.g. 1.02.
	KindInitialMargin HaircutKind = "initial_margin"
)

// Haircut is the margining applied to an ISIN.
type Haircut struct {
	Kind  HaircutKind `json:"kind"`
	Value float64     `json:"value"`
}

func (h Haircut) Validate() error {
	switch h.Kind {
	case KindHaircut:
		if !(h.Value >= 0 && h.Value < 1) {
			return fmt.Errorf("haircut %v not in [0, 1)", h.Value)
		}
	case KindInitialMargin:
		if !(h.Value > 0) || math.IsInf(h.Value, 0) {
			return fmt.Errorf("initial margin %v must be > 0", h.Value)
		}
	default:
		return fmt.Errorf("unknown haircut kind %q (want haircut or initial_margin)", string(h.Kind))
	}
	return nil
}

// RepoSnapshot is a consistent point-in-time view of the repo market,
// exported to the Rust repo-analytics crate. Maps are keyed by currency
// (gc_curves), ISIN (specials, bonds, prices, haircuts) or index name
// (fixings). Margining agreement terms (threshold, MTA) are deliberately
// not market data and are not part of the snapshot.
type RepoSnapshot struct {
	SchemaVersion int    `json:"schema_version"`
	AsOf          Date   `json:"asof"`
	GeneratedAt   string `json:"generated_at,omitempty"` // RFC 3339, informational

	GCCurves map[string]RepoCurve     `json:"gc_curves"`
	Specials map[string]SpecialSpread `json:"specials"`
	Fixings  map[string]FixingSeries  `json:"fixings"`
	Bonds    map[string]BondStatic    `json:"bonds"`
	Prices   map[string]BondPrice     `json:"prices"`
	Haircuts map[string]Haircut       `json:"haircuts"`
}

func finite(x float64) bool { return !math.IsNaN(x) && !math.IsInf(x, 0) }

// sanity bound on rates and spreads: catches percent-vs-decimal mistakes.
const maxAbsRate = 1.0

func checkRate(what string, r float64) error {
	if !finite(r) || math.Abs(r) >= maxAbsRate {
		return fmt.Errorf("%s %v is not a decimal rate (|r| < %v)", what, r, maxAbsRate)
	}
	return nil
}

// ValidateCurve checks a GC curve against the as-of date.
func ValidateCurve(asof Date, ccy string, c RepoCurve) error {
	if err := c.DayCount.Validate(); err != nil {
		return fmt.Errorf("gc_curves[%s]: %w", ccy, err)
	}
	if len(c.Pillars) == 0 {
		return fmt.Errorf("gc_curves[%s]: curve has no pillars", ccy)
	}
	prev := 0
	for _, p := range c.Pillars {
		tn, err := ParseTenor(p.Tenor)
		if err != nil {
			return fmt.Errorf("gc_curves[%s]: %w", ccy, err)
		}
		if want := tn.Days(asof.Time); p.Days != want {
			return fmt.Errorf("gc_curves[%s]: pillar %s has %d days, want %d from %s", ccy, p.Tenor, p.Days, want, asof)
		}
		if p.Days <= prev {
			return fmt.Errorf("gc_curves[%s]: pillars must be strictly increasing (%s)", ccy, p.Tenor)
		}
		prev = p.Days
		if err := checkRate(fmt.Sprintf("gc_curves[%s] %s rate", ccy, p.Tenor), p.Rate); err != nil {
			return err
		}
	}
	return nil
}

// ValidateFixings checks an overnight fixing series against the as-of date.
func ValidateFixings(asof Date, index string, f FixingSeries) error {
	if f.Currency == "" {
		return fmt.Errorf("fixings[%s]: missing currency", index)
	}
	if err := f.DayCount.Validate(); err != nil {
		return fmt.Errorf("fixings[%s]: %w", index, err)
	}
	for i, x := range f.Fixings {
		if i > 0 && !x.Date.After(f.Fixings[i-1].Date.Time) {
			return fmt.Errorf("fixings[%s]: dates must be strictly increasing (%s)", index, x.Date)
		}
		if x.Date.After(asof.Time) {
			return fmt.Errorf("fixings[%s]: fixing %s is after as-of %s", index, x.Date, asof)
		}
		if err := checkRate(fmt.Sprintf("fixings[%s] %s", index, x.Date), x.Rate); err != nil {
			return err
		}
	}
	return nil
}

// ValidateBond checks bond static data against the as-of date.
func ValidateBond(asof Date, isin string, b BondStatic) error {
	if b.Currency == "" {
		return fmt.Errorf("bonds[%s]: missing currency", isin)
	}
	switch b.Frequency {
	case 1, 2, 4, 12:
	default:
		return fmt.Errorf("bonds[%s]: coupon frequency %d not in {1, 2, 4, 12}", isin, b.Frequency)
	}
	if !finite(b.CouponRate) || b.CouponRate < 0 || b.CouponRate >= maxAbsRate {
		return fmt.Errorf("bonds[%s]: coupon rate %v must be a non-negative decimal", isin, b.CouponRate)
	}
	if !finite(b.Face) || b.Face <= 0 {
		return fmt.Errorf("bonds[%s]: face %v must be positive", isin, b.Face)
	}
	if err := b.DayCount.Validate(); err != nil {
		return fmt.Errorf("bonds[%s]: %w", isin, err)
	}
	if !b.Maturity.After(asof.Time) {
		return fmt.Errorf("bonds[%s]: matured on %s (as-of %s)", isin, b.Maturity, asof)
	}
	return nil
}

// Validate checks the whole snapshot, including cross-references: every
// special, price and haircut must refer to a known bond, and every special
// needs a GC curve in its bond's currency. All problems are reported.
func (s *RepoSnapshot) Validate() error {
	var errs []error
	add := func(err error) {
		if err != nil {
			errs = append(errs, err)
		}
	}
	if s.SchemaVersion != RepoSchemaVersion {
		add(fmt.Errorf("schema_version %d, want %d", s.SchemaVersion, RepoSchemaVersion))
	}
	if s.AsOf.IsZero() {
		add(errors.New("missing asof"))
		return errors.Join(errs...)
	}
	for _, ccy := range sortedKeys(s.GCCurves) {
		add(ValidateCurve(s.AsOf, ccy, s.GCCurves[ccy]))
	}
	for _, idx := range sortedKeys(s.Fixings) {
		add(ValidateFixings(s.AsOf, idx, s.Fixings[idx]))
	}
	for _, isin := range sortedKeys(s.Bonds) {
		add(ValidateBond(s.AsOf, isin, s.Bonds[isin]))
	}
	for _, isin := range sortedKeys(s.Specials) {
		b, ok := s.Bonds[isin]
		if !ok {
			add(fmt.Errorf("specials[%s]: no bond static for this ISIN", isin))
			continue
		}
		if _, ok := s.GCCurves[b.Currency]; !ok {
			add(fmt.Errorf("specials[%s]: no GC curve for currency %s", isin, b.Currency))
		}
		add(checkRate(fmt.Sprintf("specials[%s] spread", isin), s.Specials[isin].Spread))
	}
	for _, isin := range sortedKeys(s.Prices) {
		if _, ok := s.Bonds[isin]; !ok {
			add(fmt.Errorf("prices[%s]: no bond static for this ISIN", isin))
		}
		if p := s.Prices[isin].Clean; !finite(p) || p <= 0 {
			add(fmt.Errorf("prices[%s]: clean price %v must be positive", isin, p))
		}
	}
	for _, isin := range sortedKeys(s.Haircuts) {
		if _, ok := s.Bonds[isin]; !ok {
			add(fmt.Errorf("haircuts[%s]: no bond static for this ISIN", isin))
		}
		if err := s.Haircuts[isin].Validate(); err != nil {
			add(fmt.Errorf("haircuts[%s]: %w", isin, err))
		}
	}
	return errors.Join(errs...)
}

// MarshalRepoSnapshot validates and serialises a snapshot as indented
// JSON. Map keys are sorted, so the output is deterministic.
func MarshalRepoSnapshot(s RepoSnapshot) ([]byte, error) {
	if err := s.Validate(); err != nil {
		return nil, fmt.Errorf("invalid repo snapshot: %w", err)
	}
	b, err := json.MarshalIndent(s, "", "  ")
	if err != nil {
		return nil, err
	}
	return append(b, '\n'), nil
}

// UnmarshalRepoSnapshot parses and validates a snapshot. Unknown fields
// are rejected to catch schema drift.
func UnmarshalRepoSnapshot(data []byte) (RepoSnapshot, error) {
	var s RepoSnapshot
	dec := json.NewDecoder(bytes.NewReader(data))
	dec.DisallowUnknownFields()
	if err := dec.Decode(&s); err != nil {
		return RepoSnapshot{}, fmt.Errorf("parse repo snapshot: %w", err)
	}
	if err := s.Validate(); err != nil {
		return RepoSnapshot{}, fmt.Errorf("invalid repo snapshot: %w", err)
	}
	return s, nil
}

func sortedKeys[V any](m map[string]V) []string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	return keys
}
