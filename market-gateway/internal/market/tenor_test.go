package market

import (
	"bytes"
	"encoding/json"
	"flag"
	"os"
	"path/filepath"
	"testing"
	"time"
)

// Regenerate golden files with: go test ./internal/market -update
var update = flag.Bool("update", false, "rewrite golden files in testdata/")

func date(y int, m time.Month, d int) time.Time {
	return time.Date(y, m, d, 0, 0, 0, 0, time.UTC)
}

// checkGolden compares got with testdata/name, or rewrites it with -update.
// The Rust crate reads the same files (repo-analytics/tests/gateway_snapshot.rs).
func checkGolden(t *testing.T, name string, got []byte) {
	t.Helper()
	path := filepath.Join("testdata", name)
	if *update {
		if err := os.MkdirAll("testdata", 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, got, 0o644); err != nil {
			t.Fatal(err)
		}
		return
	}
	want, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read golden %s (run with -update to create): %v", path, err)
	}
	if !bytes.Equal(got, want) {
		t.Errorf("%s is out of date; run `go test ./internal/market -update` and re-run the Rust tests\n got: %s", path, got)
	}
}

func TestParseTenor(t *testing.T) {
	ok := map[string]Tenor{
		"ON": {1, TenorDays}, "o/n": {1, TenorDays}, "TN": {2, TenorDays},
		"1W": {1, TenorWeeks}, " 3m ": {3, TenorMonths}, "10D": {10, TenorDays},
		"1Y": {1, TenorYears},
	}
	for s, want := range ok {
		got, err := ParseTenor(s)
		if err != nil || got != want {
			t.Errorf("ParseTenor(%q) = %v, %v; want %v", s, got, err, want)
		}
	}
	for _, s := range []string{"", "M", "3X", "0M", "-1M", "1.5M", "SN"} {
		if _, err := ParseTenor(s); err == nil {
			t.Errorf("ParseTenor(%q) should fail", s)
		}
	}
}

func TestTenorEndDateClampsMonthEnd(t *testing.T) {
	cases := []struct {
		asof  time.Time
		tenor string
		want  time.Time
	}{
		{date(2026, 1, 31), "1M", date(2026, 2, 28)},
		{date(2028, 1, 31), "1M", date(2028, 2, 29)}, // leap year
		{date(2026, 8, 31), "6M", date(2027, 2, 28)},
		{date(2028, 2, 29), "1Y", date(2029, 2, 28)},
		{date(2026, 12, 15), "3M", date(2027, 3, 15)},
		{date(2026, 11, 9), "1W", date(2026, 11, 16)},
	}
	for _, c := range cases {
		tn, _ := ParseTenor(c.tenor)
		if got := tn.EndDate(c.asof); !got.Equal(c.want) {
			t.Errorf("%s + %s = %s; want %s", c.asof.Format(dateLayout), c.tenor, got.Format(dateLayout), c.want.Format(dateLayout))
		}
	}
}

// TestTenorGolden writes the cross-language tenor table that the Rust
// tests check against curve::Tenor::end_date.
func TestTenorGolden(t *testing.T) {
	type tenorCase struct {
		AsOf  string `json:"asof"`
		Tenor string `json:"tenor"`
		End   string `json:"end"`
		Days  int    `json:"days"`
	}
	asofs := []time.Time{
		date(2026, 11, 9), date(2026, 1, 31), date(2028, 1, 31),
		date(2028, 2, 29), date(2026, 8, 31), date(2026, 3, 30),
	}
	tenors := []string{"ON", "TN", "1W", "2W", "1M", "2M", "3M", "6M", "9M", "1Y", "2Y"}
	var cases []tenorCase
	for _, a := range asofs {
		for _, s := range tenors {
			tn, err := ParseTenor(s)
			if err != nil {
				t.Fatal(err)
			}
			cases = append(cases, tenorCase{
				AsOf: a.Format(dateLayout), Tenor: s,
				End: tn.EndDate(a).Format(dateLayout), Days: tn.Days(a),
			})
		}
	}
	got, err := json.MarshalIndent(cases, "", "  ")
	if err != nil {
		t.Fatal(err)
	}
	checkGolden(t, "tenor_cases.golden.json", append(got, '\n'))
}
