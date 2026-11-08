//! Classic (sale-and-repurchase) repo trade economics.
//!
//! Coupons paid on the collateral during the term are passed back to the
//! collateral giver (manufactured payments). They therefore do not change the
//! repurchase price.

use chrono::NaiveDate;

use crate::bond::Bond;
use crate::daycount::DayCount;
use crate::error::{check_dates, RepoError, Result};

/// Side of the trade, seen from the book holding it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Sell collateral and buy it back later: borrow cash.
    Repo,
    /// Buy collateral and sell it back later: lend cash.
    ReverseRepo,
}

/// Collateral margining at inception.
///
/// With the `serde` feature it reads the snapshot form
/// `{"kind": "haircut", "value": 0.02}` / `{"kind": "initial_margin", "value": 1.02}`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(tag = "kind", content = "value", rename_all = "snake_case")
)]
pub enum Margin {
    /// Cash = MV × (1 − h), e.g. `Haircut(0.02)`.
    Haircut(f64),
    /// Cash = MV / m, e.g. `InitialMargin(1.02)`.
    InitialMargin(f64),
}

impl Margin {
    /// Checks the haircut is in [0, 1) or the initial margin is positive.
    pub fn validate(self) -> Result<()> {
        match self {
            Margin::Haircut(h) if !(0.0..1.0).contains(&h) => Err(RepoError::InvalidInput(
                format!("haircut {h} not in [0, 1)"),
            )),
            Margin::InitialMargin(m) if !(m > 0.0 && m.is_finite()) => Err(
                RepoError::InvalidInput(format!("initial margin {m} must be > 0")),
            ),
            _ => Ok(()),
        }
    }

    /// Collateral value required to back `cash`.
    pub fn required_collateral(self, cash: f64) -> f64 {
        match self {
            Margin::Haircut(h) => cash / (1.0 - h),
            Margin::InitialMargin(m) => cash * m,
        }
    }

    /// Cash lent against collateral worth `collateral_value`.
    pub fn cash_against(self, collateral_value: f64) -> f64 {
        match self {
            Margin::Haircut(h) => collateral_value * (1.0 - h),
            Margin::InitialMargin(m) => collateral_value / m,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepoTrade {
    pub direction: Direction,
    pub collateral: Bond,
    /// Face amount of collateral, e.g. `10_000_000.0`.
    pub nominal: f64,
    /// Collateral clean price at the start date, per `collateral.face`.
    pub clean_price: f64,
    pub start: NaiveDate,
    /// `None` for an open (callable) repo.
    pub end: Option<NaiveDate>,
    /// Annual repo rate, simple interest.
    pub repo_rate: f64,
    pub day_count: DayCount,
    pub margin: Margin,
}

impl RepoTrade {
    /// Checks the trade's static data.
    pub fn validate(&self) -> Result<()> {
        if self.nominal <= 0.0 {
            return Err(RepoError::InvalidInput("nominal must be positive".into()));
        }
        if self.clean_price <= 0.0 {
            return Err(RepoError::InvalidInput(
                "clean price must be positive".into(),
            ));
        }
        self.margin.validate()?;
        if let Some(end) = self.end {
            check_dates(self.start, end)?;
        }
        if self.start >= self.collateral.maturity {
            return Err(RepoError::InvalidInput(
                "collateral matures before repo start".into(),
            ));
        }
        Ok(())
    }

    /// Maturity date, or an error for an open repo.
    pub fn maturity(&self) -> Result<NaiveDate> {
        self.end.ok_or(RepoError::MissingEndDate)
    }

    pub fn term_days(&self) -> Result<i64> {
        Ok((self.maturity()? - self.start).num_days())
    }

    /// Market value of the collateral for a clean price at `asof`.
    pub fn collateral_value(&self, clean: f64, asof: NaiveDate) -> Result<f64> {
        Ok(self.collateral.dirty_price(clean, asof)? * self.nominal / self.collateral.face)
    }

    /// Collateral market value at the start date.
    pub fn collateral_market_value(&self) -> Result<f64> {
        self.collateral_value(self.clean_price, self.start)
    }

    /// Cash exchanged at the start date (the purchase price).
    pub fn cash_amount(&self) -> Result<f64> {
        Ok(self.margin.cash_against(self.collateral_market_value()?))
    }

    /// Repo interest from the start date to `to`.
    pub fn repo_interest(&self, to: NaiveDate) -> Result<f64> {
        if to < self.start {
            return Err(RepoError::InvalidDates {
                start: self.start,
                end: to,
            });
        }
        Ok(self.cash_amount()? * self.repo_rate * self.day_count.year_frac(self.start, to))
    }

    /// Interest accrued at `asof`, capped at the maturity of a term repo.
    pub fn accrued_repo_interest(&self, asof: NaiveDate) -> Result<f64> {
        let to = match self.end {
            Some(end) => asof.min(end),
            None => asof,
        };
        self.repo_interest(to.max(self.start))
    }

    /// Cash repaid if the repo terminates on `end` (needed for open repos).
    pub fn repurchase_price_at(&self, end: NaiveDate) -> Result<f64> {
        check_dates(self.start, end)?;
        Ok(self.cash_amount()? + self.repo_interest(end)?)
    }

    /// Cash repaid at maturity: cash × (1 + r·τ).
    pub fn repurchase_price(&self) -> Result<f64> {
        self.repurchase_price_at(self.maturity()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    /// Zero-coupon collateral so that clean = dirty = 101.50.
    fn trade(margin: Margin, end: Option<NaiveDate>) -> RepoTrade {
        RepoTrade {
            direction: Direction::Repo,
            collateral: Bond::new(0.0, 2, d(2030, 6, 15), DayCount::Act365F).unwrap(),
            nominal: 10_000_000.0,
            clean_price: 101.50,
            start: d(2026, 1, 5),
            end,
            repo_rate: 0.045,
            day_count: DayCount::Act360,
            margin,
        }
    }

    #[test]
    fn hand_checked_economics() {
        let t = trade(Margin::Haircut(0.0), Some(d(2026, 2, 4)));
        t.validate().unwrap();
        assert_eq!(t.term_days().unwrap(), 30);
        assert!((t.cash_amount().unwrap() - 10_150_000.0).abs() < 1e-6);
        let interest = t.repo_interest(d(2026, 2, 4)).unwrap();
        assert!((interest - 38_062.50).abs() < 1e-6);
        assert!((t.repurchase_price().unwrap() - 10_188_062.50).abs() < 1e-6);
        // Accrual is capped at maturity.
        let late = t.accrued_repo_interest(d(2026, 3, 1)).unwrap();
        assert!((late - interest).abs() < 1e-9);
    }

    #[test]
    fn haircut_and_initial_margin() {
        let h = trade(Margin::Haircut(0.02), Some(d(2026, 2, 4)));
        assert!((h.cash_amount().unwrap() - 10_150_000.0 * 0.98).abs() < 1e-6);
        let m = trade(Margin::InitialMargin(1.02), Some(d(2026, 2, 4)));
        assert!((m.cash_amount().unwrap() - 10_150_000.0 / 1.02).abs() < 1e-6);
        assert!(
            (Margin::Haircut(0.02).required_collateral(h.cash_amount().unwrap()) - 10_150_000.0)
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn open_repo_needs_end_date() {
        let t = trade(Margin::Haircut(0.0), None);
        t.validate().unwrap();
        assert_eq!(t.repurchase_price(), Err(RepoError::MissingEndDate));
        assert!((t.repurchase_price_at(d(2026, 2, 4)).unwrap() - 10_188_062.50).abs() < 1e-6);
    }

    #[test]
    fn validation() {
        assert!(trade(Margin::Haircut(1.0), None).validate().is_err());
        assert!(trade(Margin::InitialMargin(0.0), None).validate().is_err());
        assert!(trade(Margin::Haircut(0.0), Some(d(2026, 1, 5)))
            .validate()
            .is_err());
    }
}
