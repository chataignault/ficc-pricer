package market

import (
	"math"
	"time"
)

// Simulated collateral used by LoadSimulatedRepoMarket. The ISINs are
// illustrative; the US one matches repo-analytics/examples/repo_report.rs.
const (
	SimUST2034 = "US91282CLW90" // 4.25% Nov-2034, trades special
	SimUST2031 = "US91282CMA05" // 4.00% Oct-2031, GC
	SimDBR2034 = "DE0001102614" // 2.60% Aug-2034, margined with initial margin
)

func round6(x float64) float64 { return math.Round(x*1e6) / 1e6 }

// LoadSimulatedRepoMarket fills the manager with a deterministic repo
// market as of asof: USD and EUR GC curves, three bonds with prices and
// haircuts, one special, and ~2 months of weekday SOFR / €STR fixings
// strictly before asof.
func LoadSimulatedRepoMarket(m *Manager, asof Date) error {
	if err := m.SetRepoAsOf(asof); err != nil {
		return err
	}
	steps := []func() error{
		func() error {
			return m.UpdateGCCurve("USD", Act360, []TenorQuote{
				{"ON", 0.0365}, {"1W", 0.0366}, {"1M", 0.0368}, {"3M", 0.0372}, {"6M", 0.0375},
			})
		},
		func() error {
			return m.UpdateGCCurve("EUR", Act360, []TenorQuote{
				{"ON", 0.0192}, {"1W", 0.0193}, {"1M", 0.0195}, {"3M", 0.0198}, {"6M", 0.0200},
			})
		},
		func() error {
			return m.UpdateBond(SimUST2034, BondStatic{Currency: "USD", CouponRate: 0.0425, Frequency: 2,
				Maturity: NewDate(time.Date(2034, 11, 15, 0, 0, 0, 0, time.UTC)), Face: 100, DayCount: Act365F})
		},
		func() error {
			return m.UpdateBond(SimUST2031, BondStatic{Currency: "USD", CouponRate: 0.04, Frequency: 2,
				Maturity: NewDate(time.Date(2031, 10, 31, 0, 0, 0, 0, time.UTC)), Face: 100, DayCount: Act365F})
		},
		func() error {
			return m.UpdateBond(SimDBR2034, BondStatic{Currency: "EUR", CouponRate: 0.026, Frequency: 1,
				Maturity: NewDate(time.Date(2034, 8, 15, 0, 0, 0, 0, time.UTC)), Face: 100, DayCount: Act365F})
		},
		func() error { return m.UpdateBondPrice(SimUST2034, 98.75) },
		func() error { return m.UpdateBondPrice(SimUST2031, 99.40) },
		func() error { return m.UpdateBondPrice(SimDBR2034, 99.10) },
		func() error { return m.UpdateHaircut(SimUST2034, Haircut{KindHaircut, 0.02}) },
		func() error { return m.UpdateHaircut(SimUST2031, Haircut{KindHaircut, 0.02}) },
		func() error { return m.UpdateHaircut(SimDBR2034, Haircut{KindInitialMargin, 1.02}) },
		func() error { return m.UpdateSpecial(SimUST2034, 0.0040) },
	}
	for _, step := range steps {
		if err := step(); err != nil {
			return err
		}
	}
	// Weekday fixings over the 60 calendar days before asof.
	i := 0
	for d := asof.AddDate(0, 0, -60); d.Before(asof.Time); d = d.AddDate(0, 0, 1) {
		if wd := d.Weekday(); wd == time.Saturday || wd == time.Sunday {
			continue
		}
		day := NewDate(d)
		if err := m.AddFixing("SOFR", "USD", Act360, Fixing{day, round6(0.0362 + 0.00001*float64(i))}); err != nil {
			return err
		}
		if err := m.AddFixing("ESTR", "EUR", Act360, Fixing{day, round6(0.0190 + 0.000005*float64(i))}); err != nil {
			return err
		}
		i++
	}
	return nil
}
