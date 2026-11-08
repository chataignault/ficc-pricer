//! Mark-to-market, margining and rate sensitivities of repo trades.

use chrono::NaiveDate;

use crate::daycount::DayCount;
use crate::error::{check_dates, Result};
use crate::trade::{Direction, RepoTrade};

const BP: f64 = 1e-4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarkToMarket {
    /// Current market value of the collateral (dirty).
    pub collateral_value: f64,
    /// Cash plus accrued repo interest owed by the collateral giver.
    pub cash_owed: f64,
    /// Collateral value needed to keep the initial haircut / margin.
    pub required_collateral: f64,
    /// Credit exposure of the book holding the trade. Positive means at risk.
    /// For a reverse repo (cash lender) this is `cash_owed − collateral_value`.
    /// For a repo (cash borrower) it is the excess collateral given away.
    pub net_exposure: f64,
}

pub fn mark_to_market(
    trade: &RepoTrade,
    asof: NaiveDate,
    current_clean: f64,
) -> Result<MarkToMarket> {
    let collateral_value = trade.collateral_value(current_clean, asof)?;
    let cash_owed = trade.cash_amount()? + trade.accrued_repo_interest(asof)?;
    let lender_exposure = cash_owed - collateral_value;
    Ok(MarkToMarket {
        collateral_value,
        cash_owed,
        required_collateral: trade.margin.required_collateral(cash_owed),
        net_exposure: match trade.direction {
            Direction::ReverseRepo => lender_exposure,
            Direction::Repo => -lender_exposure,
        },
    })
}

/// Direction of a margin transfer, seen from the book holding the trade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginFlow {
    /// The book receives collateral (or cash) from the counterparty.
    Receive,
    /// The book delivers collateral (or cash) to the counterparty.
    Pay,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MarginCall {
    pub flow: MarginFlow,
    pub amount: f64,
}

/// Variation margin needed to restore the initial haircut or margin ratio.
///
/// The call amount is the part of the shortfall (or excess) beyond
/// `threshold`. The function returns `None` if the shortfall is within the
/// threshold or the call is below the minimum transfer amount.
pub fn margin_call(
    trade: &RepoTrade,
    asof: NaiveDate,
    current_clean: f64,
    threshold: f64,
    min_transfer: f64,
) -> Result<Option<MarginCall>> {
    let mtm = mark_to_market(trade, asof, current_clean)?;
    // Positive: the collateral giver must top up.
    let deficit = mtm.required_collateral - mtm.collateral_value;
    let amount = deficit.abs() - threshold;
    if amount <= 0.0 || amount < min_transfer {
        return Ok(None);
    }
    let giver_tops_up = deficit > 0.0;
    let flow = match (trade.direction, giver_tops_up) {
        (Direction::ReverseRepo, true) | (Direction::Repo, false) => MarginFlow::Receive,
        (Direction::ReverseRepo, false) | (Direction::Repo, true) => MarginFlow::Pay,
    };
    Ok(Some(MarginCall { flow, amount }))
}

/// Change in the repurchase price for a +1bp move in the repo rate
/// (`cash × τ × 1bp`). The sign is from the book's point of view: a repo pays
/// more interest, so its DV01 is negative.
pub fn repo_dv01(trade: &RepoTrade) -> Result<f64> {
    let tau = trade.day_count.year_frac(trade.start, trade.maturity()?);
    let dv01 = trade.cash_amount()? * tau * BP;
    Ok(match trade.direction {
        Direction::ReverseRepo => dv01,
        Direction::Repo => -dv01,
    })
}

/// Collateral value change for a 1bp fall in yield, on the trade's nominal.
pub fn collateral_dv01(trade: &RepoTrade, asof: NaiveDate, current_clean: f64) -> Result<f64> {
    let bond = &trade.collateral;
    let y = bond.yield_from_price(current_clean, asof)?;
    Ok(bond.dv01(y, asof)? * trade.nominal / bond.face)
}

/// Specialness: how far a bond's special rate trades below GC.
pub fn specialness(gc_rate: f64, special_rate: f64) -> f64 {
    gc_rate - special_rate
}

/// Funding saved by borrowing `cash` against special collateral instead of GC.
pub fn funding_advantage(
    gc_rate: f64,
    special_rate: f64,
    cash: f64,
    start: NaiveDate,
    end: NaiveDate,
    dc: DayCount,
) -> Result<f64> {
    check_dates(start, end)?;
    Ok(cash * specialness(gc_rate, special_rate) * dc.year_frac(start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bond::Bond;
    use crate::trade::Margin;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn trade(direction: Direction, haircut: f64) -> RepoTrade {
        RepoTrade {
            direction,
            collateral: Bond::new(0.0, 2, d(2030, 6, 15), DayCount::Act365F).unwrap(),
            nominal: 10_000_000.0,
            clean_price: 100.0,
            start: d(2026, 1, 5),
            end: Some(d(2026, 2, 4)),
            repo_rate: 0.036,
            day_count: DayCount::Act360,
            margin: Margin::Haircut(haircut),
        }
    }

    #[test]
    fn mtm_at_inception_is_overcollateralised() {
        let t = trade(Direction::ReverseRepo, 0.02);
        let m = mark_to_market(&t, t.start, 100.0).unwrap();
        assert!((m.collateral_value - 10_000_000.0).abs() < 1e-6);
        assert!((m.cash_owed - 9_800_000.0).abs() < 1e-6);
        assert!((m.net_exposure + 200_000.0).abs() < 1e-6);
        assert!((m.required_collateral - m.collateral_value).abs() < 1e-6);
    }

    #[test]
    fn margin_call_after_price_drop() {
        let t = trade(Direction::ReverseRepo, 0.02);
        // Ten days later the price falls to 99.
        let asof = d(2026, 1, 15);
        let cash_owed = 9_800_000.0 * (1.0 + 0.036 * 10.0 / 360.0);
        let expected = cash_owed / 0.98 - 9_900_000.0;
        let call = margin_call(&t, asof, 99.0, 0.0, 0.0).unwrap().unwrap();
        assert_eq!(call.flow, MarginFlow::Receive);
        assert!((call.amount - expected).abs() < 1e-6);
        // The same move seen from the repo side is a payment.
        let call = margin_call(&trade(Direction::Repo, 0.02), asof, 99.0, 0.0, 0.0)
            .unwrap()
            .unwrap();
        assert_eq!(call.flow, MarginFlow::Pay);
        // Inside the threshold: no call.
        assert!(margin_call(&t, asof, 99.0, 200_000.0, 0.0)
            .unwrap()
            .is_none());
        // Below the minimum transfer amount: no call.
        assert!(margin_call(&t, asof, 99.0, 0.0, 1e9).unwrap().is_none());
    }

    #[test]
    fn excess_collateral_is_returned() {
        let t = trade(Direction::ReverseRepo, 0.02);
        let call = margin_call(&t, d(2026, 1, 15), 101.0, 0.0, 0.0)
            .unwrap()
            .unwrap();
        assert_eq!(call.flow, MarginFlow::Pay);
    }

    #[test]
    fn dv01s() {
        let t = trade(Direction::ReverseRepo, 0.0);
        assert!((repo_dv01(&t).unwrap() - 10_000_000.0 * 30.0 / 360.0 * 1e-4).abs() < 1e-9);
        assert!(repo_dv01(&trade(Direction::Repo, 0.0)).unwrap() < 0.0);
        assert!(collateral_dv01(&t, t.start, 100.0).unwrap() > 0.0);
    }

    #[test]
    fn specials() {
        assert!((specialness(0.0365, 0.0300) - 0.0065).abs() < 1e-15);
        let adv = funding_advantage(
            0.0365,
            0.03,
            10_000_000.0,
            d(2026, 1, 5),
            d(2026, 2, 4),
            DayCount::Act360,
        )
        .unwrap();
        assert!((adv - 10_000_000.0 * 0.0065 * 30.0 / 360.0).abs() < 1e-6);
    }
}
