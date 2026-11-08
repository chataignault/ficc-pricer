//! Repo term structure: GC curve, forward repo rates, compounded overnight
//! rates and per-ISIN specials.

use std::collections::HashMap;

use chrono::{Days, Months, NaiveDate};

use crate::daycount::DayCount;
use crate::error::{check_dates, RepoError, Result};

/// A money-market tenor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tenor {
    Days(u32),
    Weeks(u32),
    Months(u32),
    Years(u32),
}

impl Tenor {
    /// Parses `ON`, `O/N`, `TN`, or `<n>D`/`<n>W`/`<n>M`/`<n>Y`.
    /// `ON` is one day and `TN` two (spot conventions are ignored).
    pub fn parse(s: &str) -> Result<Tenor> {
        let u = s.trim().to_ascii_uppercase();
        match u.as_str() {
            "ON" | "O/N" => return Ok(Tenor::Days(1)),
            "TN" | "T/N" => return Ok(Tenor::Days(2)),
            _ => {}
        }
        let err = || RepoError::UnknownTenor(s.to_string());
        if u.len() < 2 {
            return Err(err());
        }
        let (num, unit) = u.split_at(u.len() - 1);
        let n: u32 = num.parse().map_err(|_| err())?;
        if n == 0 {
            return Err(err());
        }
        match unit {
            "D" => Ok(Tenor::Days(n)),
            "W" => Ok(Tenor::Weeks(n)),
            "M" => Ok(Tenor::Months(n)),
            "Y" => Ok(Tenor::Years(n)),
            _ => Err(err()),
        }
    }

    /// Calendar date of the tenor from `asof` (no business-day adjustment).
    pub fn end_date(self, asof: NaiveDate) -> NaiveDate {
        match self {
            Tenor::Days(n) => asof + Days::new(n as u64),
            Tenor::Weeks(n) => asof + Days::new(7 * n as u64),
            Tenor::Months(n) => asof + Months::new(n),
            Tenor::Years(n) => asof + Months::new(12 * n),
        }
    }
}

/// Simple-rate repo curve. Pillars are (days from `asof`, annual simple rate).
/// Rates are interpolated linearly and extrapolated flat.
#[derive(Debug, Clone, PartialEq)]
pub struct RepoCurve {
    pub asof: NaiveDate,
    pub day_count: DayCount,
    pillars: Vec<(i64, f64)>,
}

impl RepoCurve {
    pub fn new(asof: NaiveDate, day_count: DayCount, pillars: Vec<(i64, f64)>) -> Result<Self> {
        if pillars.is_empty() {
            return Err(RepoError::EmptyCurve);
        }
        if pillars[0].0 <= 0 || pillars.windows(2).any(|w| w[1].0 <= w[0].0) {
            return Err(RepoError::UnsortedPillars);
        }
        Ok(RepoCurve {
            asof,
            day_count,
            pillars,
        })
    }

    /// Builds a curve from quoted tenors, e.g. `[("ON", 0.0365), ("1M", 0.0370)]`.
    pub fn from_tenors(
        asof: NaiveDate,
        day_count: DayCount,
        quotes: &[(&str, f64)],
    ) -> Result<Self> {
        let pillars = quotes
            .iter()
            .map(|(t, r)| Ok(((Tenor::parse(t)?.end_date(asof) - asof).num_days(), *r)))
            .collect::<Result<Vec<_>>>()?;
        Self::new(asof, day_count, pillars)
    }

    pub fn pillars(&self) -> &[(i64, f64)] {
        &self.pillars
    }

    /// Term repo rate for a maturity `days` after `asof`.
    pub fn rate(&self, days: i64) -> f64 {
        let p = &self.pillars;
        if days <= p[0].0 {
            return p[0].1;
        }
        if days >= p[p.len() - 1].0 {
            return p[p.len() - 1].1;
        }
        let i = p.partition_point(|(d, _)| *d < days);
        let ((d0, r0), (d1, r1)) = (p[i - 1], p[i]);
        r0 + (r1 - r0) * (days - d0) as f64 / (d1 - d0) as f64
    }

    pub fn rate_at(&self, date: NaiveDate) -> f64 {
        self.rate((date - self.asof).num_days())
    }

    fn year_frac(&self, days: i64) -> f64 {
        self.day_count
            .year_frac(self.asof, self.asof + chrono::Duration::days(days))
    }

    /// Discount factor `1 / (1 + r·τ)` to `days` after `asof`.
    pub fn discount_factor(&self, days: i64) -> f64 {
        1.0 / (1.0 + self.rate(days) * self.year_frac(days))
    }

    /// Simple forward repo rate between `d1` and `d2` days from `asof`.
    pub fn forward_rate(&self, d1: i64, d2: i64) -> Result<f64> {
        if d2 <= d1 || d1 < 0 {
            return Err(RepoError::InvalidInput(format!(
                "forward period {d1}..{d2} days"
            )));
        }
        let df1 = if d1 == 0 {
            1.0
        } else {
            self.discount_factor(d1)
        };
        let df2 = self.discount_factor(d2);
        let tau = self.year_frac(d2) - self.year_frac(d1);
        Ok((df1 / df2 - 1.0) / tau)
    }
}

/// Annualised simple rate from compounding overnight fixings over
/// `[start, end)`, SOFR/€STR style. Each fixing holds until the next fixing
/// date, so a Friday fixing covers the weekend. `fixings` must be sorted
/// and include a fixing on or before `start`.
pub fn compounded_overnight(
    fixings: &[(NaiveDate, f64)],
    start: NaiveDate,
    end: NaiveDate,
    dc: DayCount,
) -> Result<f64> {
    check_dates(start, end)?;
    if fixings.windows(2).any(|w| w[1].0 <= w[0].0) {
        return Err(RepoError::UnsortedPillars);
    }
    // Last fixing on or before start.
    let first = fixings
        .iter()
        .rposition(|(d, _)| *d <= start)
        .ok_or(RepoError::MissingFixing(start))?;
    let mut growth = 1.0;
    for i in first..fixings.len() {
        let (date, rate) = fixings[i];
        if date >= end {
            break;
        }
        let from = date.max(start);
        let to = fixings.get(i + 1).map_or(end, |(d, _)| (*d).min(end));
        growth *= 1.0 + rate * dc.year_frac(from, to);
    }
    Ok((growth - 1.0) / dc.year_frac(start, end))
}

/// GC curve plus a specialness spread per ISIN.
/// `special_rate = GC rate − spread`.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecialsCurve {
    pub gc: RepoCurve,
    spreads: HashMap<String, f64>,
}

impl SpecialsCurve {
    pub fn new(gc: RepoCurve) -> Self {
        SpecialsCurve {
            gc,
            spreads: HashMap::new(),
        }
    }

    /// Sets the specialness of `isin` (positive means it trades below GC).
    pub fn set_spread(&mut self, isin: impl Into<String>, spread: f64) {
        self.spreads.insert(isin.into(), spread);
    }

    pub fn spread(&self, isin: &str) -> f64 {
        self.spreads.get(isin).copied().unwrap_or(0.0)
    }

    /// Repo rate for `isin`. An ISIN with no spread gets the GC rate.
    pub fn special_rate(&self, isin: &str, days: i64) -> f64 {
        self.gc.rate(days) - self.spread(isin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn curve() -> RepoCurve {
        RepoCurve::from_tenors(
            d(2026, 1, 5),
            DayCount::Act360,
            &[
                ("ON", 0.0360),
                ("1W", 0.0362),
                ("1M", 0.0365),
                ("3M", 0.0370),
            ],
        )
        .unwrap()
    }

    #[test]
    fn tenors() {
        assert_eq!(Tenor::parse("on").unwrap(), Tenor::Days(1));
        assert_eq!(Tenor::parse("3M").unwrap(), Tenor::Months(3));
        assert!(Tenor::parse("3X").is_err());
        assert!(Tenor::parse("M").is_err());
        assert_eq!(Tenor::Months(1).end_date(d(2026, 1, 5)), d(2026, 2, 5));
        assert_eq!(
            curve().pillars(),
            &[(1, 0.0360), (7, 0.0362), (31, 0.0365), (90, 0.0370)]
        );
    }

    #[test]
    fn interpolation_and_extrapolation() {
        let c = curve();
        assert_eq!(c.rate(0), 0.0360);
        assert_eq!(c.rate(31), 0.0365);
        assert_eq!(c.rate(365), 0.0370);
        let mid = c.rate(19); // halfway between 7 and 31
        assert!((mid - 0.03635).abs() < 1e-15);
    }

    #[test]
    fn discount_factor_reprices_pillars() {
        let c = curve();
        for &(days, r) in c.pillars() {
            let df = c.discount_factor(days);
            let implied = (1.0 / df - 1.0) / (days as f64 / 360.0);
            assert!((implied - r).abs() < 1e-14);
        }
        // Forward from 0 equals the spot rate.
        assert!((c.forward_rate(0, 31).unwrap() - 0.0365).abs() < 1e-14);
        // Upward-sloping curve: forward above spot.
        assert!(c.forward_rate(31, 90).unwrap() > c.rate(90));
    }

    #[test]
    fn invalid_curves() {
        let a = d(2026, 1, 5);
        assert_eq!(
            RepoCurve::new(a, DayCount::Act360, vec![]),
            Err(RepoError::EmptyCurve)
        );
        assert_eq!(
            RepoCurve::new(a, DayCount::Act360, vec![(30, 0.03), (7, 0.03)]),
            Err(RepoError::UnsortedPillars)
        );
    }

    #[test]
    fn compounded_overnight_weekend() {
        // Fri 2026-01-09 fixing covers 3 days.
        let fixings = [
            (d(2026, 1, 8), 0.0360),
            (d(2026, 1, 9), 0.0370),
            (d(2026, 1, 12), 0.0350),
        ];
        let r = compounded_overnight(&fixings, d(2026, 1, 8), d(2026, 1, 13), DayCount::Act360)
            .unwrap();
        let growth = (1.0 + 0.036 / 360.0) * (1.0 + 0.037 * 3.0 / 360.0) * (1.0 + 0.035 / 360.0);
        assert!((r - (growth - 1.0) / (5.0 / 360.0)).abs() < 1e-14);
        // One day at a flat fixing returns the fixing; daily fixings over a
        // month compound above it.
        let one = [(d(2026, 1, 1), 0.04)];
        let r = compounded_overnight(&one, d(2026, 1, 1), d(2026, 1, 2), DayCount::Act360).unwrap();
        assert!((r - 0.04).abs() < 1e-12);
        let daily: Vec<_> = (0..30)
            .map(|i| (d(2026, 1, 1) + Days::new(i), 0.04))
            .collect();
        let r =
            compounded_overnight(&daily, d(2026, 1, 1), d(2026, 1, 31), DayCount::Act360).unwrap();
        let expected = ((1.0 + 0.04 / 360.0_f64).powi(30) - 1.0) / (30.0 / 360.0);
        assert!(r > 0.04 && (r - expected).abs() < 1e-12);
        assert!(
            compounded_overnight(&fixings, d(2026, 1, 1), d(2026, 1, 13), DayCount::Act360)
                .is_err()
        );
    }

    #[test]
    fn specials() {
        let mut s = SpecialsCurve::new(curve());
        s.set_spread("US91282CJZ59", 0.0050);
        assert!((s.special_rate("US91282CJZ59", 31) - 0.0315).abs() < 1e-15);
        assert_eq!(s.special_rate("UNKNOWN", 31), 0.0365);
    }
}
