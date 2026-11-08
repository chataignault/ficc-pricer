//! Fixed-coupon bond used as repo collateral.
//!
//! Prices are quoted per `face` (typically 100). Accrued interest uses the
//! fraction of the current coupon period elapsed, with the day counts of the
//! bond's convention (ACT conventions give ACT/ACT ICMA accrual).

use chrono::{Months, NaiveDate};

use crate::daycount::DayCount;
use crate::error::{RepoError, Result};

const BP: f64 = 1e-4;

#[derive(Debug, Clone, PartialEq)]
pub struct Bond {
    /// Annual coupon rate, e.g. `0.0425` for 4.25%.
    pub coupon_rate: f64,
    /// Coupons per year (1, 2, 4 or 12).
    pub frequency: u32,
    pub maturity: NaiveDate,
    /// Price basis and redemption amount (usually 100).
    pub face: f64,
    pub day_count: DayCount,
}

impl Bond {
    pub fn new(
        coupon_rate: f64,
        frequency: u32,
        maturity: NaiveDate,
        day_count: DayCount,
    ) -> Result<Self> {
        if ![1, 2, 4, 12].contains(&frequency) {
            return Err(RepoError::InvalidInput(format!(
                "coupon frequency {frequency} not in {{1, 2, 4, 12}}"
            )));
        }
        if coupon_rate < 0.0 {
            return Err(RepoError::InvalidInput("negative coupon rate".into()));
        }
        Ok(Bond {
            coupon_rate,
            frequency,
            maturity,
            face: 100.0,
            day_count,
        })
    }

    /// Coupon paid on each coupon date, per `face`.
    pub fn coupon_amount(&self) -> f64 {
        self.face * self.coupon_rate / self.frequency as f64
    }

    fn coupon_date(&self, periods_before_maturity: u32) -> NaiveDate {
        let months = periods_before_maturity * (12 / self.frequency);
        self.maturity
            .checked_sub_months(Months::new(months))
            .expect("coupon date out of range")
    }

    /// `[previous coupon, next coupon, ..., maturity]` around `settle`,
    /// with `previous <= settle < next`. Dates roll back from maturity.
    fn schedule_from(&self, settle: NaiveDate) -> Result<Vec<NaiveDate>> {
        if settle >= self.maturity {
            return Err(RepoError::InvalidInput(format!(
                "settlement {settle} on or after bond maturity {}",
                self.maturity
            )));
        }
        let mut dates = Vec::new();
        let mut k = 0;
        loop {
            let d = self.coupon_date(k);
            dates.push(d);
            if d <= settle {
                break;
            }
            k += 1;
        }
        dates.reverse();
        Ok(dates)
    }

    /// Last coupon date on or before `settle`.
    pub fn previous_coupon(&self, settle: NaiveDate) -> Result<NaiveDate> {
        Ok(self.schedule_from(settle)?[0])
    }

    /// First coupon date strictly after `settle`.
    pub fn next_coupon(&self, settle: NaiveDate) -> Result<NaiveDate> {
        Ok(self.schedule_from(settle)?[1])
    }

    /// Coupon dates in the interval `(start, end]`.
    pub fn coupons_between(&self, start: NaiveDate, end: NaiveDate) -> Result<Vec<NaiveDate>> {
        let end = end.min(self.maturity);
        if end <= start {
            return Ok(Vec::new());
        }
        Ok(self
            .schedule_from(start)?
            .into_iter()
            .skip(1)
            .filter(|d| *d <= end)
            .collect())
    }

    /// Accrued interest per `face` at `settle`.
    pub fn accrued_interest(&self, settle: NaiveDate) -> Result<f64> {
        if settle == self.maturity {
            return Ok(0.0);
        }
        let s = self.schedule_from(settle)?;
        let (prev, next) = (s[0], s[1]);
        let frac =
            self.day_count.days(prev, settle) as f64 / self.day_count.days(prev, next) as f64;
        Ok(self.coupon_amount() * frac)
    }

    pub fn dirty_price(&self, clean: f64, settle: NaiveDate) -> Result<f64> {
        Ok(clean + self.accrued_interest(settle)?)
    }

    pub fn clean_price(&self, dirty: f64, settle: NaiveDate) -> Result<f64> {
        Ok(dirty - self.accrued_interest(settle)?)
    }

    /// Dirty price and its derivative with respect to the yield.
    /// Street convention: yield compounded at the coupon frequency.
    fn dirty_and_derivative(&self, y: f64, settle: NaiveDate) -> Result<(f64, f64)> {
        let s = self.schedule_from(settle)?;
        let (prev, next) = (s[0], s[1]);
        let f = self.frequency as f64;
        let w = self.day_count.days(settle, next) as f64 / self.day_count.days(prev, next) as f64;
        let base = 1.0 + y / f;
        if base <= 0.0 {
            return Err(RepoError::InvalidInput(format!("yield {y} too negative")));
        }
        let n = s.len() - 1;
        let (mut pv, mut dpv) = (0.0, 0.0);
        for k in 0..n {
            let cf = self.coupon_amount() + if k == n - 1 { self.face } else { 0.0 };
            let t = k as f64 + w;
            let disc = base.powf(-t);
            pv += cf * disc;
            dpv += -cf * t / f * disc / base;
        }
        Ok((pv, dpv))
    }

    pub fn dirty_price_from_yield(&self, y: f64, settle: NaiveDate) -> Result<f64> {
        Ok(self.dirty_and_derivative(y, settle)?.0)
    }

    pub fn clean_price_from_yield(&self, y: f64, settle: NaiveDate) -> Result<f64> {
        self.clean_price(self.dirty_price_from_yield(y, settle)?, settle)
    }

    /// Yield to maturity from a clean price (Newton, bisection fallback).
    pub fn yield_from_price(&self, clean: f64, settle: NaiveDate) -> Result<f64> {
        let target = self.dirty_price(clean, settle)?;
        let tol = 1e-12;

        let mut y = self.coupon_rate.max(0.01);
        for _ in 0..50 {
            let (pv, dpv) = match self.dirty_and_derivative(y, settle) {
                Ok(v) => v,
                Err(_) => break,
            };
            let diff = pv - target;
            if diff.abs() < tol {
                return Ok(y);
            }
            if dpv.abs() < 1e-14 {
                break;
            }
            y -= diff / dpv;
        }

        // Bisection: price is decreasing in yield.
        let (mut lo, mut hi) = (-0.5, 2.0);
        let f_lo = self.dirty_price_from_yield(lo, settle)? - target;
        let f_hi = self.dirty_price_from_yield(hi, settle)? - target;
        if f_lo * f_hi > 0.0 {
            return Err(RepoError::NoConvergence);
        }
        for _ in 0..200 {
            let mid = 0.5 * (lo + hi);
            let f_mid = self.dirty_price_from_yield(mid, settle)? - target;
            if f_mid.abs() < tol || (hi - lo) < 1e-14 {
                return Ok(mid);
            }
            if f_mid > 0.0 {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Err(RepoError::NoConvergence)
    }

    /// Modified duration at yield `y`.
    pub fn modified_duration(&self, y: f64, settle: NaiveDate) -> Result<f64> {
        let (pv, dpv) = self.dirty_and_derivative(y, settle)?;
        Ok(-dpv / pv)
    }

    /// Price change per `face` for a 1bp fall in yield.
    pub fn dv01(&self, y: f64, settle: NaiveDate) -> Result<f64> {
        let (_, dpv) = self.dirty_and_derivative(y, settle)?;
        Ok(-dpv * BP)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn bond(dc: DayCount) -> Bond {
        Bond::new(0.06, 2, d(2030, 6, 15), dc).unwrap()
    }

    #[test]
    fn schedule() {
        let b = bond(DayCount::Act365F);
        assert_eq!(b.previous_coupon(d(2026, 3, 15)).unwrap(), d(2025, 12, 15));
        assert_eq!(b.next_coupon(d(2026, 3, 15)).unwrap(), d(2026, 6, 15));
        assert_eq!(b.next_coupon(d(2026, 6, 15)).unwrap(), d(2026, 12, 15));
        assert_eq!(
            b.coupons_between(d(2026, 3, 15), d(2027, 1, 1)).unwrap(),
            vec![d(2026, 6, 15), d(2026, 12, 15)]
        );
        assert!(b
            .coupons_between(d(2026, 3, 15), d(2026, 6, 14))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn accrued_interest() {
        // 30/360: 90 of 180 days of a 3.00 coupon.
        let ai = bond(DayCount::Thirty360)
            .accrued_interest(d(2026, 3, 15))
            .unwrap();
        assert!((ai - 1.5).abs() < 1e-12);
        // ACT: 90 of 182 actual days.
        let ai = bond(DayCount::Act365F)
            .accrued_interest(d(2026, 3, 15))
            .unwrap();
        assert!((ai - 3.0 * 90.0 / 182.0).abs() < 1e-12);
        // Zero on a coupon date.
        let ai = bond(DayCount::Act365F)
            .accrued_interest(d(2026, 6, 15))
            .unwrap();
        assert_eq!(ai, 0.0);
    }

    #[test]
    fn par_bond_on_coupon_date() {
        let b = bond(DayCount::Act365F);
        let p = b.clean_price_from_yield(0.06, d(2026, 6, 15)).unwrap();
        assert!((p - 100.0).abs() < 1e-10);
    }

    #[test]
    fn yield_price_round_trip() {
        let b = bond(DayCount::Act365F);
        let settle = d(2026, 9, 3);
        for y in [-0.005, 0.0, 0.03, 0.06, 0.12] {
            let p = b.clean_price_from_yield(y, settle).unwrap();
            let y2 = b.yield_from_price(p, settle).unwrap();
            assert!((y - y2).abs() < 1e-10, "y={y} y2={y2}");
        }
    }

    #[test]
    fn dv01_matches_bump() {
        let b = bond(DayCount::Act365F);
        let settle = d(2026, 9, 3);
        let y = 0.045;
        let bumped = b.dirty_price_from_yield(y - 0.5e-4, settle).unwrap()
            - b.dirty_price_from_yield(y + 0.5e-4, settle).unwrap();
        assert!((b.dv01(y, settle).unwrap() - bumped).abs() < 1e-8);
        assert!(b.modified_duration(y, settle).unwrap() > 0.0);
    }
}
