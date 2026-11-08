//! Analytics report for a sample 1M term repo on a US Treasury.
//!
//! Run with `cargo run --example repo_report`.

use chrono::NaiveDate;
use repo_analytics::{
    carry::{carry, forward_clean_price, implied_repo_rate},
    compounded_overnight,
    risk::{
        collateral_dv01, funding_advantage, margin_call, mark_to_market, repo_dv01, specialness,
    },
    Bond, DayCount, Direction, Margin, RepoCurve, RepoTrade, Result, SpecialsCurve,
};

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).expect("valid date")
}

fn main() -> Result<()> {
    let asof = d(2026, 11, 9);
    let isin = "US91282CLW90";

    // Market: GC curve plus a specialness spread for the collateral.
    let gc = RepoCurve::from_tenors(
        asof,
        DayCount::Act360,
        &[
            ("ON", 0.0365),
            ("1W", 0.0366),
            ("1M", 0.0368),
            ("3M", 0.0372),
            ("6M", 0.0375),
        ],
    )?;
    let mut specials = SpecialsCurve::new(gc.clone());
    specials.set_spread(isin, 0.0040);

    // Trade: borrow cash for 1M against a 4.25% Nov-2034 Treasury (coupon on 15 Nov).
    let bond = Bond::new(0.0425, 2, d(2034, 11, 15), DayCount::Act365F)?;
    let end = d(2026, 12, 9);
    let term = (end - asof).num_days();
    let trade = RepoTrade {
        direction: Direction::Repo,
        collateral: bond.clone(),
        nominal: 50_000_000.0,
        clean_price: 98.75,
        start: asof,
        end: Some(end),
        repo_rate: specials.special_rate(isin, term),
        day_count: DayCount::Act360,
        margin: Margin::Haircut(0.02),
    };
    trade.validate()?;

    println!("== Trade economics ({isin}, {asof} -> {end}, {term} days)");
    println!(
        "  repo rate                {:>16.4}%",
        trade.repo_rate * 100.0
    );
    println!(
        "  accrued interest /100    {:>16.6}",
        bond.accrued_interest(asof)?
    );
    println!(
        "  dirty price              {:>16.6}",
        bond.dirty_price(trade.clean_price, asof)?
    );
    println!(
        "  collateral MV            {:>16.2}",
        trade.collateral_market_value()?
    );
    println!("  cash amount (2% haircut) {:>16.2}", trade.cash_amount()?);
    println!(
        "  repo interest            {:>16.2}",
        trade.repo_interest(end)?
    );
    println!(
        "  repurchase price         {:>16.2}",
        trade.repurchase_price()?
    );

    println!("\n== Implied repo & carry (per 100 face)");
    let dirty = bond.dirty_price(trade.clean_price, asof)?;
    let fwd_clean = forward_clean_price(
        &bond,
        trade.clean_price,
        asof,
        end,
        trade.repo_rate,
        trade.day_count,
    )?;
    println!(
        "  coupons in term          {:>16?}",
        bond.coupons_between(asof, end)?
    );
    println!("  forward clean (breakeven){:>16.6}", fwd_clean);
    let quoted_fwd_clean = 98.60;
    let quoted_fwd_dirty = bond.dirty_price(quoted_fwd_clean, end)?;
    let irr = implied_repo_rate(&bond, dirty, quoted_fwd_dirty, asof, end, trade.day_count)?;
    println!(
        "  implied repo @ fwd {quoted_fwd_clean:.2}  {:>16.4}%",
        irr * 100.0
    );
    let c = carry(
        &bond,
        trade.clean_price,
        asof,
        end,
        trade.repo_rate,
        trade.day_count,
    )?;
    println!("  coupon income            {:>16.6}", c.coupon_income);
    println!("  financing cost           {:>16.6}", c.financing_cost);
    println!("  net carry                {:>16.6}", c.net_carry);

    println!("\n== Risk & margining");
    let mid = d(2026, 11, 23);
    let shocked = 97.90;
    let mtm = mark_to_market(&trade, mid, shocked)?;
    println!("  MTM on {mid} at clean {shocked:.2}");
    println!("    collateral value       {:>16.2}", mtm.collateral_value);
    println!("    cash owed              {:>16.2}", mtm.cash_owed);
    println!(
        "    required collateral    {:>16.2}",
        mtm.required_collateral
    );
    println!("    net exposure           {:>16.2}", mtm.net_exposure);
    match margin_call(&trade, mid, shocked, 50_000.0, 100_000.0)? {
        Some(call) => println!(
            "    margin call            {:>16.2} ({:?})",
            call.amount, call.flow
        ),
        None => println!("    margin call                        none"),
    }
    println!("  repo DV01                {:>16.2}", repo_dv01(&trade)?);
    println!(
        "  collateral DV01          {:>16.2}",
        collateral_dv01(&trade, asof, trade.clean_price)?
    );
    let gc_rate = gc.rate(term);
    println!(
        "  specialness              {:>15.1}bp",
        specialness(gc_rate, trade.repo_rate) * 1e4
    );
    println!(
        "  funding advantage vs GC  {:>16.2}",
        funding_advantage(
            gc_rate,
            trade.repo_rate,
            trade.cash_amount()?,
            asof,
            end,
            trade.day_count
        )?
    );

    println!("\n== Term structure (GC, ACT/360)");
    for &(days, r) in gc.pillars() {
        println!(
            "  {days:>4}d  rate {:>8.4}%  DF {:.8}",
            r * 100.0,
            gc.discount_factor(days)
        );
    }
    println!(
        "  1Mx3M forward            {:>15.4}%",
        gc.forward_rate(gc.pillars()[2].0, gc.pillars()[3].0)? * 100.0
    );
    println!(
        "  3Mx6M forward            {:>15.4}%",
        gc.forward_rate(gc.pillars()[3].0, gc.pillars()[4].0)? * 100.0
    );

    // Rolling O/N over the term vs the 1M term rate, with simulated fixings
    // on business days.
    let fixings: Vec<_> = (0..term)
        .map(|i| asof + chrono::Days::new(i as u64))
        .filter(|day| {
            !matches!(
                chrono::Datelike::weekday(day),
                chrono::Weekday::Sat | chrono::Weekday::Sun
            )
        })
        .enumerate()
        .map(|(i, day)| (day, 0.0365 + 0.00001 * i as f64))
        .collect();
    let rolled = compounded_overnight(&fixings, asof, end, DayCount::Act360)?;
    println!("  rolled O/N (compounded)  {:>15.4}%", rolled * 100.0);
    println!("  1M term GC               {:>15.4}%", gc_rate * 100.0);
    println!(
        "  term premium             {:>15.2}bp",
        (gc_rate - rolled) * 1e4
    );
    Ok(())
}
