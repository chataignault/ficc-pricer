package market

import (
	"fmt"
	"sync"
	"time"

	"go.uber.org/zap"
)

// SpotRate represents a currency pair spot rate
type SpotRate struct {
	Pair      string    // e.g., "EUR/USD"
	Rate      float64   // e.g., 1.1050
	Timestamp time.Time // Last update time
}

// DiscountCurve represents a discount curve for a currency
// TODO: Expand to support pillar points and interpolation methods
type DiscountCurve struct {
	Currency    string
	FlatRate    float64 // Simplified: single flat rate for now
	Compounding string  // "continuous", "annual", etc.
	Timestamp   time.Time
}

// VolSurface represents a volatility surface for a currency pair
// TODO: Expand to support volatility grids
type VolSurface struct {
	Pair        string
	FlatVol     float64 // Simplified: single flat volatility for now
	Timestamp   time.Time
}

// MarketSnapshot represents a point-in-time view of market data
type MarketSnapshot struct {
	SpotRates       map[string]SpotRate       // Key: "EUR/USD"
	DiscountCurves  map[string]DiscountCurve  // Key: "USD"
	VolSurfaces     map[string]VolSurface     // Key: "EUR/USD"
	SnapshotTime    time.Time
}

// Manager manages market data state with thread-safe access
type Manager struct {
	mu              sync.RWMutex
	spotRates       map[string]SpotRate
	discountCurves  map[string]DiscountCurve
	volSurfaces     map[string]VolSurface
	logger          *zap.Logger
}

// NewManager creates a new market data manager
func NewManager(logger *zap.Logger) *Manager {
	if logger == nil {
		logger, _ = zap.NewDevelopment()
	}

	return &Manager{
		spotRates:      make(map[string]SpotRate),
		discountCurves: make(map[string]DiscountCurve),
		volSurfaces:    make(map[string]VolSurface),
		logger:         logger,
	}
}

// UpdateSpotRate updates a spot rate for a currency pair
func (m *Manager) UpdateSpotRate(pair string, rate float64) error {
	if rate <= 0 {
		return fmt.Errorf("invalid rate %f for pair %s: must be positive", rate, pair)
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	m.spotRates[pair] = SpotRate{
		Pair:      pair,
		Rate:      rate,
		Timestamp: time.Now(),
	}

	m.logger.Info("updated spot rate",
		zap.String("pair", pair),
		zap.Float64("rate", rate),
	)

	return nil
}

// GetSpotRate retrieves a spot rate for a currency pair
func (m *Manager) GetSpotRate(pair string) (SpotRate, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	spot, exists := m.spotRates[pair]
	if !exists {
		return SpotRate{}, fmt.Errorf("spot rate not found for pair %s", pair)
	}

	return spot, nil
}

// UpdateDiscountCurve updates a discount curve for a currency
func (m *Manager) UpdateDiscountCurve(currency string, flatRate float64, compounding string) error {
	if flatRate < 0 {
		return fmt.Errorf("invalid flat rate %f for currency %s: must be non-negative", flatRate, currency)
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	m.discountCurves[currency] = DiscountCurve{
		Currency:    currency,
		FlatRate:    flatRate,
		Compounding: compounding,
		Timestamp:   time.Now(),
	}

	m.logger.Info("updated discount curve",
		zap.String("currency", currency),
		zap.Float64("flat_rate", flatRate),
		zap.String("compounding", compounding),
	)

	return nil
}

// GetDiscountCurve retrieves a discount curve for a currency
func (m *Manager) GetDiscountCurve(currency string) (DiscountCurve, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	curve, exists := m.discountCurves[currency]
	if !exists {
		return DiscountCurve{}, fmt.Errorf("discount curve not found for currency %s", currency)
	}

	return curve, nil
}

// UpdateVolSurface updates a volatility surface for a currency pair
func (m *Manager) UpdateVolSurface(pair string, flatVol float64) error {
	if flatVol < 0 || flatVol > 1 {
		return fmt.Errorf("invalid flat volatility %f for pair %s: must be between 0 and 1", flatVol, pair)
	}

	m.mu.Lock()
	defer m.mu.Unlock()

	m.volSurfaces[pair] = VolSurface{
		Pair:      pair,
		FlatVol:   flatVol,
		Timestamp: time.Now(),
	}

	m.logger.Info("updated vol surface",
		zap.String("pair", pair),
		zap.Float64("flat_vol", flatVol),
	)

	return nil
}

// GetVolSurface retrieves a volatility surface for a currency pair
func (m *Manager) GetVolSurface(pair string) (VolSurface, error) {
	m.mu.RLock()
	defer m.mu.RUnlock()

	surface, exists := m.volSurfaces[pair]
	if !exists {
		return VolSurface{}, fmt.Errorf("vol surface not found for pair %s", pair)
	}

	return surface, nil
}

// GetSnapshot creates a point-in-time snapshot of all market data
func (m *Manager) GetSnapshot() MarketSnapshot {
	m.mu.RLock()
	defer m.mu.RUnlock()

	// Deep copy to avoid concurrent access issues
	spotRates := make(map[string]SpotRate, len(m.spotRates))
	for k, v := range m.spotRates {
		spotRates[k] = v
	}

	discountCurves := make(map[string]DiscountCurve, len(m.discountCurves))
	for k, v := range m.discountCurves {
		discountCurves[k] = v
	}

	volSurfaces := make(map[string]VolSurface, len(m.volSurfaces))
	for k, v := range m.volSurfaces {
		volSurfaces[k] = v
	}

	return MarketSnapshot{
		SpotRates:      spotRates,
		DiscountCurves: discountCurves,
		VolSurfaces:    volSurfaces,
		SnapshotTime:   time.Now(),
	}
}

// LoadSnapshot loads a complete market snapshot (replaces current state)
func (m *Manager) LoadSnapshot(snapshot MarketSnapshot) {
	m.mu.Lock()
	defer m.mu.Unlock()

	m.spotRates = snapshot.SpotRates
	m.discountCurves = snapshot.DiscountCurves
	m.volSurfaces = snapshot.VolSurfaces

	m.logger.Info("loaded market snapshot",
		zap.Int("spot_rates", len(snapshot.SpotRates)),
		zap.Int("discount_curves", len(snapshot.DiscountCurves)),
		zap.Int("vol_surfaces", len(snapshot.VolSurfaces)),
		zap.Time("snapshot_time", snapshot.SnapshotTime),
	)
}

// Stats returns statistics about the current market data
func (m *Manager) Stats() map[string]int {
	m.mu.RLock()
	defer m.mu.RUnlock()

	return map[string]int{
		"spot_rates":      len(m.spotRates),
		"discount_curves": len(m.discountCurves),
		"vol_surfaces":    len(m.volSurfaces),
	}
}
