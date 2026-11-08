//! Repo analytics driven by a market-gateway snapshot.
//!
//! ```text
//! cargo run --features serde --example repo_report_from_snapshot [snapshot.json | -]
//! market-gateway repo export --asof 2026-11-09 | \
//!     cargo run --features serde --example repo_report_from_snapshot -- -
//! ```
//!
//! With no argument it reads the gateway's golden test snapshot.

use std::fs::File;
use std::io::{self, BufReader};

use chrono::{Days, Months};
use repo_analytics::{
    carry::carry,
    risk::{collateral_dv01, funding_advantage, margin_call, repo_dv01, specialness},
    Direction, RepoMarket, Result,
};

const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../market-gateway/internal/market/testdata/repo_snapshot.golden.json"
);

fn load(arg: Option<String>) -> Result<RepoMarket> {
    match arg.as_deref() {
        Some("-") => RepoMarket::from_reader(io::stdin().lock()),
        path => {
            let path = path.unwrap_or(GOLDEN);
            let file = File::open(path).map_err(|e| {
                repo_analytics::RepoError::InvalidSnapshot(format!("open {path}: {e}"))
            })?;
            RepoMarket::from_reader(BufReader::new(file))
        }
    }
}

fn main() -> Result<()> {
    let market = load(std::env::args().nth(1))?;
    let asof = market.asof;
    let end = asof + Months::new(1);
    let nominal = 50_000_000.0;
    println!("== Repo market as of {asof}: 1M repos ({asof} -> {end}) on {nominal:.0} face\n");

    println!(
        "{:<14} {:>4} {:>8} {:>8} {:>7} {:>16} {:>14} {:>12} {:>12}",
        "ISIN", "ccy", "clean", "rate %", "spec bp", "cash", "repurchase", "net carry", "repo DV01"
    );
    for isin in market.isins() {
        let c = market.collateral(isin)?;
        let trade = match market.repo_trade(isin, Direction::Repo, nominal, Some(end)) {
            Ok(t) => t,
            Err(e) => {
                println!("{isin:<14} skipped: {e}");
                continue;
            }
        };
        let gc = market.gc_curve(&c.currency)?.rate_at(end);
        let cy = carry(
            &c.bond,
            trade.clean_price,
            asof,
            end,
            trade.repo_rate,
            trade.day_count,
        )?;
        println!(
            "{:<14} {:>4} {:>8.3} {:>8.4} {:>7.1} {:>16.2} {:>14.2} {:>12.6} {:>12.2}",
            isin,
            c.currency,
            trade.clean_price,
            trade.repo_rate * 100.0,
            specialness(gc, trade.repo_rate) * 1e4,
            trade.cash_amount()?,
            trade.repurchase_price()?,
            cy.net_carry,
            repo_dv01(&trade)?,
        );
    }

    // Risk on the most special bond.
    let (isin, spread) = market
        .isins()
        .into_iter()
        .filter_map(|i| {
            let ccy = &market.collateral(i).ok()?.currency;
            Some((i, market.specials(ccy).ok()?.spread(i)))
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .expect("snapshot has bonds");
    let trade = market.repo_trade(isin, Direction::Repo, nominal, Some(end))?;
    let ccy = &market.collateral(isin)?.currency;
    let gc = market.gc_curve(ccy)?.rate_at(end);
    println!("\n== Risk: {isin} (special by {:.1}bp)", spread * 1e4);
    println!(
        "  collateral DV01          {:>16.2}",
        collateral_dv01(&trade, asof, trade.clean_price)?
    );
    println!(
        "  funding advantage vs GC  {:>16.2}",
        funding_advantage(
            gc,
            trade.repo_rate,
            trade.cash_amount()?,
            asof,
            end,
            trade.day_count
        )?
    );
    let mid = asof + Days::new(14);
    let shocked = trade.clean_price - 0.85;
    match margin_call(&trade, mid, shocked, 50_000.0, 100_000.0)? {
        Some(call) => println!(
            "  margin call on {mid} at {shocked:.2}: {:.2} ({:?})",
            call.amount, call.flow
        ),
        None => println!("  no margin call on {mid} at {shocked:.2}"),
    }

    println!("\n== GC curves vs realised overnight (last month)");
    for (ccy, index) in [("USD", "SOFR"), ("EUR", "ESTR")] {
        let (Ok(curve), Ok(_)) = (market.gc_curve(ccy), market.fixings(index)) else {
            continue;
        };
        let start = asof - Months::new(1);
        let realised = market.compounded(index, start, asof)?;
        println!(
            "  {ccy}  ON {:.4}%  1M {:.4}%  3Mx6M fwd {:.4}%  | {index} compounded {start} -> {asof}: {:.4}%",
            curve.rate(1) * 100.0,
            curve.rate_at(end) * 100.0,
            curve.forward_rate(curve.pillars()[3].0, curve.pillars()[4].0)? * 100.0,
            realised * 100.0,
        );
    }
    Ok(())
}
