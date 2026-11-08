package market

import (
	"fmt"
	"strconv"
	"strings"
	"time"
)

// TenorUnit is the unit of a money-market tenor.
type TenorUnit int

const (
	TenorDays TenorUnit = iota
	TenorWeeks
	TenorMonths
	TenorYears
)

// Tenor is a money-market tenor such as ON, 1W, 3M or 1Y.
//
// Its semantics mirror repo-analytics `curve::Tenor` exactly. The Rust pricer
// checks that the pillar days sent by the gateway match its own tenor
// arithmetic, so the two must not diverge.
type Tenor struct {
	N    int
	Unit TenorUnit
}

// ParseTenor parses ON, O/N, TN, T/N or <n>D/<n>W/<n>M/<n>Y (case-insensitive).
// ON is one day and TN two: spot lags and business days are ignored.
func ParseTenor(s string) (Tenor, error) {
	u := strings.ToUpper(strings.TrimSpace(s))
	switch u {
	case "ON", "O/N":
		return Tenor{1, TenorDays}, nil
	case "TN", "T/N":
		return Tenor{2, TenorDays}, nil
	}
	bad := fmt.Errorf("unknown tenor: %q", s)
	if len(u) < 2 {
		return Tenor{}, bad
	}
	n, err := strconv.ParseUint(u[:len(u)-1], 10, 32)
	if err != nil || n == 0 {
		return Tenor{}, bad
	}
	var unit TenorUnit
	switch u[len(u)-1] {
	case 'D':
		unit = TenorDays
	case 'W':
		unit = TenorWeeks
	case 'M':
		unit = TenorMonths
	case 'Y':
		unit = TenorYears
	default:
		return Tenor{}, bad
	}
	return Tenor{int(n), unit}, nil
}

// EndDate is the calendar date of the tenor from asof, with no business-day
// adjustment. Month arithmetic clamps to the end of the month
// (Jan 31 + 1M = Feb 28), as chrono does, unlike time.AddDate which
// would roll over to March.
func (t Tenor) EndDate(asof time.Time) time.Time {
	switch t.Unit {
	case TenorDays:
		return asof.AddDate(0, 0, t.N)
	case TenorWeeks:
		return asof.AddDate(0, 0, 7*t.N)
	case TenorMonths:
		return addMonthsClamped(asof, t.N)
	default:
		return addMonthsClamped(asof, 12*t.N)
	}
}

// Days from asof to the tenor end date.
func (t Tenor) Days(asof time.Time) int {
	return daysBetween(asof, t.EndDate(asof))
}

func addMonthsClamped(d time.Time, months int) time.Time {
	y, m, day := d.Date()
	first := time.Date(y, m+time.Month(months), 1, 0, 0, 0, 0, time.UTC)
	last := first.AddDate(0, 1, -1).Day()
	if day > last {
		day = last
	}
	return time.Date(first.Year(), first.Month(), day, 0, 0, 0, 0, time.UTC)
}

// daysBetween counts calendar days between two dates, ignoring time of day.
func daysBetween(a, b time.Time) int {
	ua := time.Date(a.Year(), a.Month(), a.Day(), 0, 0, 0, 0, time.UTC)
	ub := time.Date(b.Year(), b.Month(), b.Day(), 0, 0, 0, 0, time.UTC)
	return int(ub.Sub(ua).Hours() / 24)
}
