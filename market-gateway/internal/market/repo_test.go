package market

import (
	"reflect"
	"strings"
	"sync"
	"testing"

	"go.uber.org/zap"
)

var goldenAsOf = NewDate(date(2026, 11, 9))

func simulated(t *testing.T) (*Manager, RepoSnapshot) {
	t.Helper()
	m := NewManager(zap.NewNop())
	if err := LoadSimulatedRepoMarket(m, goldenAsOf); err != nil {
		t.Fatalf("LoadSimulatedRepoMarket: %v", err)
	}
	s, err := m.GetRepoSnapshot()
	if err != nil {
		t.Fatalf("GetRepoSnapshot: %v", err)
	}
	return m, s
}

// TestRepoSnapshotGolden pins the exported JSON. The Rust crate loads the
// same file in repo-analytics/tests/gateway_snapshot.rs.
func TestRepoSnapshotGolden(t *testing.T) {
	_, s := simulated(t)
	b, err := MarshalRepoSnapshot(s)
	if err != nil {
		t.Fatal(err)
	}
	checkGolden(t, "repo_snapshot.golden.json", b)
}

func TestRepoSnapshotRoundTrip(t *testing.T) {
	_, s := simulated(t)
	b, err := MarshalRepoSnapshot(s)
	if err != nil {
		t.Fatal(err)
	}
	back, err := UnmarshalRepoSnapshot(b)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(s, back) {
		t.Fatalf("round trip changed the snapshot")
	}
	// Loading it into a fresh manager gives the same snapshot.
	m2 := NewManager(zap.NewNop())
	if err := m2.LoadRepoSnapshot(back); err != nil {
		t.Fatal(err)
	}
	again, _ := m2.GetRepoSnapshot()
	if !reflect.DeepEqual(s, again) {
		t.Fatalf("load/get changed the snapshot")
	}
}

func TestSimulatedMarketContents(t *testing.T) {
	m, s := simulated(t)
	usd := s.GCCurves["USD"]
	wantDays := []int{1, 7, 30, 92, 181} // from 2026-11-09
	for i, p := range usd.Pillars {
		if p.Days != wantDays[i] {
			t.Errorf("USD %s days = %d, want %d", p.Tenor, p.Days, wantDays[i])
		}
	}
	sofr := s.Fixings["SOFR"].Fixings
	if len(sofr) == 0 || !sofr[len(sofr)-1].Date.Before(goldenAsOf.Time) {
		t.Errorf("SOFR fixings must end strictly before as-of")
	}
	for _, f := range sofr {
		if wd := f.Date.Weekday(); wd == 0 || wd == 6 {
			t.Errorf("weekend fixing %s", f.Date)
		}
	}
	stats := m.RepoStats()
	if stats["bonds"] != 3 || stats["gc_curves"] != 2 || stats["specials"] != 1 {
		t.Errorf("unexpected stats %v", stats)
	}
}

func TestRepoSnapshotValidation(t *testing.T) {
	cases := []struct {
		name   string
		mutate func(s *RepoSnapshot)
		want   string
	}{
		{"schema version", func(s *RepoSnapshot) { s.SchemaVersion = 2 }, "schema_version"},
		{"unsorted pillars", func(s *RepoSnapshot) {
			c := s.GCCurves["USD"]
			c.Pillars[1], c.Pillars[2] = c.Pillars[2], c.Pillars[1]
		}, "strictly increasing"},
		{"pillar days disagree with tenor", func(s *RepoSnapshot) { s.GCCurves["USD"].Pillars[2].Days = 31 }, "want 30"},
		{"percent instead of decimal", func(s *RepoSnapshot) { s.GCCurves["EUR"].Pillars[0].Rate = 1.92 }, "decimal rate"},
		{"bad day count", func(s *RepoSnapshot) {
			c := s.GCCurves["USD"]
			c.DayCount = "ACT/ACT"
			s.GCCurves["USD"] = c
		}, "unknown day count"},
		{"empty curve", func(s *RepoSnapshot) { s.GCCurves["USD"] = RepoCurve{DayCount: Act360} }, "no pillars"},
		{"haircut out of range", func(s *RepoSnapshot) { s.Haircuts[SimUST2034] = Haircut{KindHaircut, 1} }, "not in [0, 1)"},
		{"unknown haircut kind", func(s *RepoSnapshot) { s.Haircuts[SimUST2034] = Haircut{"pct", 0.02} }, "unknown haircut kind"},
		{"price for unknown bond", func(s *RepoSnapshot) { s.Prices["XS0000000000"] = BondPrice{100} }, "no bond static"},
		{"special without GC curve", func(s *RepoSnapshot) { delete(s.GCCurves, "USD") }, "no GC curve for currency USD"},
		{"fixing after asof", func(s *RepoSnapshot) {
			f := s.Fixings["SOFR"]
			f.Fixings = append(f.Fixings, Fixing{NewDate(date(2026, 11, 10)), 0.0365})
			s.Fixings["SOFR"] = f
		}, "after as-of"},
		{"bad frequency", func(s *RepoSnapshot) {
			b := s.Bonds[SimUST2031]
			b.Frequency = 3
			s.Bonds[SimUST2031] = b
		}, "frequency"},
		{"matured bond", func(s *RepoSnapshot) {
			b := s.Bonds[SimUST2031]
			b.Maturity = goldenAsOf
			s.Bonds[SimUST2031] = b
		}, "matured"},
	}
	for _, c := range cases {
		t.Run(c.name, func(t *testing.T) {
			_, s := simulated(t)
			c.mutate(&s)
			err := s.Validate()
			if err == nil || !strings.Contains(err.Error(), c.want) {
				t.Fatalf("Validate() = %v; want error containing %q", err, c.want)
			}
			if _, err := MarshalRepoSnapshot(s); err == nil {
				t.Fatalf("MarshalRepoSnapshot accepted an invalid snapshot")
			}
		})
	}
}

func TestUnmarshalRejectsUnknownFields(t *testing.T) {
	_, s := simulated(t)
	b, _ := MarshalRepoSnapshot(s)
	bad := strings.Replace(string(b), `"schema_version": 1,`, `"schema_version": 1, "threshold": 50000,`, 1)
	if _, err := UnmarshalRepoSnapshot([]byte(bad)); err == nil || !strings.Contains(err.Error(), "unknown field") {
		t.Fatalf("want unknown field error, got %v", err)
	}
}

func TestManagerRepoUpdateRules(t *testing.T) {
	m := NewManager(zap.NewNop())
	if err := m.UpdateGCCurve("USD", Act360, []TenorQuote{{"ON", 0.03}}); err == nil {
		t.Error("curve update before as-of should fail")
	}
	if _, err := m.GetRepoSnapshot(); err == nil {
		t.Error("snapshot before as-of should fail")
	}
	if err := m.SetRepoAsOf(goldenAsOf); err != nil {
		t.Fatal(err)
	}
	if err := m.UpdateBondPrice(SimUST2034, 99); err == nil {
		t.Error("price for unknown bond should fail")
	}
	// Quotes are sorted by maturity regardless of input order.
	if err := m.UpdateGCCurve("USD", Act360, []TenorQuote{{"3M", 0.0372}, {"ON", 0.0365}, {"1M", 0.0368}}); err != nil {
		t.Fatal(err)
	}
	s, _ := m.GetRepoSnapshot()
	if got := s.GCCurves["USD"].Pillars[0].Tenor; got != "ON" {
		t.Errorf("first pillar %s, want ON", got)
	}
	// Duplicate maturities are rejected (1M and 30D from 2026-11-09).
	if err := m.UpdateGCCurve("USD", Act360, []TenorQuote{{"1M", 0.03}, {"30D", 0.03}}); err == nil {
		t.Error("duplicate pillar days should fail")
	}
	bond := BondStatic{Currency: "USD", CouponRate: 0.04, Frequency: 2, Maturity: NewDate(date(2031, 10, 31)), Face: 100, DayCount: Act365F}
	if err := m.UpdateBond(SimUST2031, bond); err != nil {
		t.Fatal(err)
	}
	bond.Currency = "EUR"
	if err := m.UpdateBond(SimUST2031, bond); err == nil {
		t.Error("currency change should fail")
	}
	if err := m.UpdateSpecial(SimUST2031, 0.01); err != nil {
		t.Error(err)
	}
}

func TestAddFixingOrdersAndReplaces(t *testing.T) {
	m := NewManager(zap.NewNop())
	_ = m.SetRepoAsOf(goldenAsOf)
	add := func(d Date, r float64) error { return m.AddFixing("SOFR", "USD", Act360, Fixing{d, r}) }
	for _, f := range []Fixing{{NewDate(date(2026, 11, 5)), 0.03}, {NewDate(date(2026, 11, 3)), 0.01}, {NewDate(date(2026, 11, 4)), 0.02}} {
		if err := add(f.Date, f.Rate); err != nil {
			t.Fatal(err)
		}
	}
	if err := add(NewDate(date(2026, 11, 4)), 0.025); err != nil {
		t.Fatal(err)
	}
	s, _ := m.GetRepoSnapshot()
	got := s.Fixings["SOFR"].Fixings
	want := []Fixing{{NewDate(date(2026, 11, 3)), 0.01}, {NewDate(date(2026, 11, 4)), 0.025}, {NewDate(date(2026, 11, 5)), 0.03}}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("fixings = %v, want %v", got, want)
	}
	if err := add(NewDate(date(2026, 11, 10)), 0.03); err == nil {
		t.Error("future fixing should fail")
	}
	if err := m.AddFixing("SOFR", "USD", Act365F, Fixing{NewDate(date(2026, 11, 6)), 0.03}); err == nil {
		t.Error("day count change should fail")
	}
}

func TestSetRepoAsOfRollsCurve(t *testing.T) {
	m, _ := simulated(t)
	next := NewDate(date(2026, 12, 31))
	if err := m.SetRepoAsOf(next); err == nil {
		// Fixings end before the old as-of, so they stay valid; days must move.
		s, _ := m.GetRepoSnapshot()
		if got := s.GCCurves["USD"].Pillars[2].Days; got != 31 { // 2026-12-31 + 1M = 2027-01-31
			t.Errorf("1M days after roll = %d, want 31", got)
		}
	} else {
		t.Fatal(err)
	}
	// Moving back before existing fixings is rejected and leaves state intact.
	if err := m.SetRepoAsOf(NewDate(date(2026, 9, 1))); err == nil {
		t.Fatal("as-of before existing fixings should fail")
	}
	s, _ := m.GetRepoSnapshot()
	if !s.AsOf.Equal(next.Time) {
		t.Errorf("failed roll changed as-of to %s", s.AsOf)
	}
}

func TestRepoSnapshotIsDeepCopy(t *testing.T) {
	m, s := simulated(t)
	s.GCCurves["USD"].Pillars[0].Rate = 0.5
	s.Fixings["SOFR"].Fixings[0].Rate = 0.5
	s.Bonds["NEW"] = BondStatic{}
	again, _ := m.GetRepoSnapshot()
	if again.GCCurves["USD"].Pillars[0].Rate == 0.5 || again.Fixings["SOFR"].Fixings[0].Rate == 0.5 {
		t.Error("snapshot shares slices with the manager")
	}
	if _, ok := again.Bonds["NEW"]; ok {
		t.Error("snapshot shares maps with the manager")
	}
}

// Run with -race.
func TestConcurrentRepoAccess(t *testing.T) {
	m, _ := simulated(t)
	var wg sync.WaitGroup
	for w := 0; w < 4; w++ {
		wg.Add(2)
		go func(w int) {
			defer wg.Done()
			for i := 0; i < 200; i++ {
				_ = m.UpdateBondPrice(SimUST2034, 98+float64(i%10)/10)
				_ = m.UpdateGCCurve("USD", Act360, []TenorQuote{{"ON", 0.0365}, {"1M", 0.0368 + float64(w)/1e4}})
			}
		}(w)
		go func() {
			defer wg.Done()
			for i := 0; i < 200; i++ {
				s, err := m.GetRepoSnapshot()
				if err != nil {
					t.Error(err)
					return
				}
				if _, err := MarshalRepoSnapshot(s); err != nil {
					t.Error(err)
					return
				}
			}
		}()
	}
	wg.Wait()
}
