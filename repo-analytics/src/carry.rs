//! Bond forward prices from repo financing, implied repo rate and carry.
//!
//! All prices are per `bond.face`. Coupons paid inside the term are
//! reinvested at the repo rate until the forward date:
//!
//! ```text
//! F_dirty = P_dirty · (1 + r·τ) − Σ cᵢ · (1 + r·τᵢ)
//! ```

use chrono::NaiveDate;

use crate::bond::Bond;
use crate::daycount::DayCount;
use crate::error::{check_dates, RepoError, Result};

/// Coupons paid in `(start, end]`, each with its reinvestment year fraction to `end`.
fn coupon_flows(
    bond: &Bond,
    start: NaiveDate,
    end: NaiveDate,
    dc: DayCount,
) -> Result<Vec<(f64, f64)>> {
    Ok(bond
        .coupons_between(start, end)?
        .into_iter()
        .map(|d| (bond.coupon_amount(), dc.year_frac(d, end)))
        .collect())
}

/// Forward dirty price of `bond` at `fwd_date` from repo financing.
pub fn forward_dirty_price(
    bond: &Bond,
    spot_dirty: f64,
    start: NaiveDate,
    fwd_date: NaiveDate,
    repo_rate: f64,
    dc: DayCount,
) -> Result<f64> {
    check_dates(start, fwd_date)?;
    let tau = dc.year_frac(start, fwd_date);
    let reinvested: f64 = coupon_flows(bond, start, fwd_date, dc)?
        .iter()
        .map(|(c, t)| c * (1.0 + repo_rate * t))
        .sum();
    Ok(spot_dirty * (1.0 + repo_rate * tau) - reinvested)
}

/// Forward clean price. This is the breakeven clean price at `fwd_date`:
/// selling there leaves a repo-financed long position flat.
pub fn forward_clean_price(
    bond: &Bond,
    spot_clean: f64,
    start: NaiveDate,
    fwd_date: NaiveDate,
    repo_rate: f64,
    dc: DayCount,
) -> Result<f64> {
    let spot_dirty = bond.dirty_price(spot_clean, start)?;
    let fwd_dirty = forward_dirty_price(bond, spot_dirty, start, fwd_date, repo_rate, dc)?;
    if fwd_date >= bond.maturity {
        return Err(RepoError::InvalidInput(
            "forward date on or after bond maturity".into(),
        ));
    }
    bond.clean_price(fwd_dirty, fwd_date)
}

/// Repo rate implied by spot and forward dirty prices (closed form, since
/// the forward price is linear in the rate):
///
/// ```text
/// r = (F − P + Σcᵢ) / (P·τ − Σ cᵢ·τᵢ)
/// ```
pub fn implied_repo_rate(
    bond: &Bond,
    spot_dirty: f64,
    fwd_dirty: f64,
    start: NaiveDate,
    end: NaiveDate,
    dc: DayCount,
) -> Result<f64> {
    check_dates(start, end)?;
    let tau = dc.year_frac(start, end);
    let flows = coupon_flows(bond, start, end, dc)?;
    let sum_c: f64 = flows.iter().map(|(c, _)| c).sum();
    let sum_ct: f64 = flows.iter().map(|(c, t)| c * t).sum();
    let denom = spot_dirty * tau - sum_ct;
    if denom.abs() < 1e-14 {
        return Err(RepoError::InvalidInput(
            "degenerate implied repo denominator".into(),
        ));
    }
    Ok((fwd_dirty - spot_dirty + sum_c) / denom)
}

/// Carry of a repo-financed long bond position over `[start, end]`, per `face`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Carry {
    /// Coupon accrual over the term (coupons received + change in accrued).
    pub coupon_income: f64,
    /// Repo interest on the dirty price.
    pub financing_cost: f64,
    /// `coupon_income − financing_cost`. Positive means positive carry.
    pub net_carry: f64,
}

pub fn carry(
    bond: &Bond,
    spot_clean: f64,
    start: NaiveDate,
    end: NaiveDate,
    repo_rate: f64,
    dc: DayCount,
) -> Result<Carry> {
    check_dates(start, end)?;
    let spot_dirty = bond.dirty_price(spot_clean, start)?;
    let coupons: f64 = bond.coupons_between(start, end)?.len() as f64 * bond.coupon_amount();
    let coupon_income =
        bond.accrued_interest(end.min(bond.maturity))? - bond.accrued_interest(start)? + coupons;
    let financing_cost = spot_dirty * repo_rate * dc.year_frac(start, end);
    Ok(Carry {
        coupon_income,
        financing_cost,
        net_carry: coupon_income - financing_cost,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn bond() -> Bond {
        Bond::new(0.05, 2, d(2030, 6, 15), DayCount::Act365F).unwrap()
    }

    #[test]
    fn forward_without_coupon() {
        let (s, e) = (d(2026, 1, 5), d(2026, 2, 4));
        let f = forward_dirty_price(&bond(), 101.5, s, e, 0.045, DayCount::Act360).unwrap();
        assert!((f - 101.5 * (1.0 + 0.045 * 30.0 / 360.0)).abs() < 1e-12);
    }

    #[test]
    fn forward_with_coupon_in_term() {
        let (s, e) = (d(2026, 6, 1), d(2026, 7, 1));
        let f = forward_dirty_price(&bond(), 102.0, s, e, 0.04, DayCount::Act360).unwrap();
        let expected = 102.0 * (1.0 + 0.04 * 30.0 / 360.0) - 2.5 * (1.0 + 0.04 * 16.0 / 360.0);
        assert!((f - expected).abs() < 1e-12);
    }

    #[test]
    fn implied_repo_round_trip() {
        let b = bond();
        for (s, e) in [
            (d(2026, 1, 5), d(2026, 2, 4)),
            (d(2026, 6, 1), d(2027, 1, 10)),
        ] {
            for r in [-0.005, 0.0, 0.025, 0.07] {
                let f = forward_dirty_price(&b, 99.8, s, e, r, DayCount::Act360).unwrap();
                let r2 = implied_repo_rate(&b, 99.8, f, s, e, DayCount::Act360).unwrap();
                assert!((r - r2).abs() < 1e-12, "r={r} r2={r2}");
            }
        }
    }

    #[test]
    fn carry_and_breakeven() {
        let b = bond();
        let (s, e) = (d(2026, 1, 5), d(2026, 2, 4)); // no coupon in term
        let c = carry(&b, 99.0, s, e, 0.03, DayCount::Act360).unwrap();
        // 5% coupon financed at 3%: positive carry.
        assert!(c.net_carry > 0.0);
        // Without coupons in the term, breakeven clean = spot clean − carry.
        let be = forward_clean_price(&b, 99.0, s, e, 0.03, DayCount::Act360).unwrap();
        assert!((be - (99.0 - c.net_carry)).abs() < 1e-12);
    }
}
