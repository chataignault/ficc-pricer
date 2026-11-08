use std::fmt;
use std::str::FromStr;

use chrono::{Datelike, NaiveDate};

use crate::error::{RepoError, Result};

/// Day count conventions used in money markets and bond accrual.
///
/// Money-market defaults: ACT/360 for USD and EUR repo, ACT/365F for GBP.
///
/// The string forms (`ACT/360`, `ACT/365F`, `30/360`) are those of the
/// market-gateway snapshot schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DayCount {
    #[cfg_attr(feature = "serde", serde(rename = "ACT/360"))]
    Act360,
    #[cfg_attr(feature = "serde", serde(rename = "ACT/365F"))]
    Act365F,
    /// 30/360 US (bond basis).
    #[cfg_attr(feature = "serde", serde(rename = "30/360"))]
    Thirty360,
}

impl fmt::Display for DayCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DayCount {
    type Err = RepoError;

    fn from_str(s: &str) -> Result<Self> {
        match s.trim().to_ascii_uppercase().as_str() {
            "ACT/360" => Ok(DayCount::Act360),
            "ACT/365F" | "ACT/365" => Ok(DayCount::Act365F),
            "30/360" => Ok(DayCount::Thirty360),
            _ => Err(RepoError::InvalidInput(format!("unknown day count {s:?}"))),
        }
    }
}

impl DayCount {
    pub fn as_str(self) -> &'static str {
        match self {
            DayCount::Act360 => "ACT/360",
            DayCount::Act365F => "ACT/365F",
            DayCount::Thirty360 => "30/360",
        }
    }

    /// Number of days between `start` and `end` under the convention.
    pub fn days(self, start: NaiveDate, end: NaiveDate) -> i64 {
        match self {
            DayCount::Act360 | DayCount::Act365F => (end - start).num_days(),
            DayCount::Thirty360 => {
                let (y1, m1, mut d1) = (start.year(), start.month() as i64, start.day() as i64);
                let (y2, m2, mut d2) = (end.year(), end.month() as i64, end.day() as i64);
                if d1 == 31 {
                    d1 = 30;
                }
                if d2 == 31 && d1 == 30 {
                    d2 = 30;
                }
                360 * (y2 - y1) as i64 + 30 * (m2 - m1) + (d2 - d1)
            }
        }
    }

    /// Days in a year under the convention.
    pub fn basis(self) -> f64 {
        match self {
            DayCount::Act360 | DayCount::Thirty360 => 360.0,
            DayCount::Act365F => 365.0,
        }
    }

    /// Year fraction between `start` and `end`.
    pub fn year_frac(self, start: NaiveDate, end: NaiveDate) -> f64 {
        self.days(start, end) as f64 / self.basis()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn act_conventions() {
        let (s, e) = (d(2026, 1, 1), d(2026, 7, 1)); // 181 days
        assert_eq!(DayCount::Act360.days(s, e), 181);
        assert!((DayCount::Act360.year_frac(s, e) - 181.0 / 360.0).abs() < 1e-15);
        assert!((DayCount::Act365F.year_frac(s, e) - 181.0 / 365.0).abs() < 1e-15);
    }

    #[test]
    fn thirty_360_end_of_month() {
        assert_eq!(
            DayCount::Thirty360.days(d(2025, 12, 31), d(2026, 3, 31)),
            90
        );
        assert_eq!(
            DayCount::Thirty360.days(d(2026, 1, 15), d(2026, 7, 15)),
            180
        );
        assert_eq!(DayCount::Thirty360.days(d(2026, 2, 28), d(2026, 3, 31)), 33);
    }

    #[test]
    fn string_forms_round_trip() {
        for dc in [DayCount::Act360, DayCount::Act365F, DayCount::Thirty360] {
            assert_eq!(dc.as_str().parse::<DayCount>().unwrap(), dc);
        }
        assert_eq!("act/365".parse::<DayCount>().unwrap(), DayCount::Act365F);
        assert!("ACT/ACT".parse::<DayCount>().is_err());
    }
}
