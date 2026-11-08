//! Cross-language checks against the golden files written by the Go
//! market-gateway tests (`go test ./internal/market -update`).
#![cfg(feature = "serde")]

use chrono::NaiveDate;
use repo_analytics::{
    compounded_overnight, market::SCHEMA_VERSION, Bond, DayCount, Direction, Margin, RepoCurve,
    RepoError, RepoMarket, RepoSnapshot, RepoTrade, Tenor,
};
use serde_json::{json, Value};

const TESTDATA: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../market-gateway/internal/market/testdata"
);

fn golden(name: &str) -> String {
    let path = format!("{TESTDATA}/{name}");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

const UST: &str = "US91282CLW90";
const UST_GC: &str = "US91282CMA05";
const DBR: &str = "DE0001102614";

fn market() -> RepoMarket {
    RepoMarket::from_json(&golden("repo_snapshot.golden.json")).expect("golden snapshot loads")
}

/// Go `Tenor.EndDate` and Rust `Tenor::end_date` must agree on every case,
/// including month-end clamping and leap years.
#[test]
fn tenor_arithmetic_matches_gateway() {
    let cases: Vec<Value> = serde_json::from_str(&golden("tenor_cases.golden.json")).unwrap();
    assert!(cases.len() > 50);
    for c in cases {
        let asof: NaiveDate = c["asof"].as_str().unwrap().parse().unwrap();
        let tenor = c["tenor"].as_str().unwrap();
        let end = Tenor::parse(tenor).unwrap().end_date(asof);
        assert_eq!(
            end.to_string(),
            c["end"].as_str().unwrap(),
            "{asof} + {tenor}"
        );
        assert_eq!(
            (end - asof).num_days(),
            c["days"].as_i64().unwrap(),
            "{asof} + {tenor}"
        );
    }
}

/// The gateway snapshot reproduces the market hard-coded in
/// examples/repo_report.rs.
#[test]
fn golden_snapshot_matches_reference_market() {
    let m = market();
    let asof = d(2026, 11, 9);
    assert_eq!(m.asof, asof);
    assert_eq!(m.currencies(), ["EUR", "USD"]);
    assert_eq!(m.isins(), [DBR, UST, UST_GC]);

    let reference = RepoCurve::from_tenors(
        asof,
        DayCount::Act360,
        &[
            ("ON", 0.0365),
            ("1W", 0.0366),
            ("1M", 0.0368),
            ("3M", 0.0372),
            ("6M", 0.0375),
        ],
    )
    .unwrap();
    assert_eq!(m.gc_curve("USD").unwrap(), &reference);

    let c = m.collateral(UST).unwrap();
    assert_eq!(c.currency, "USD");
    assert_eq!(
        c.bond,
        Bond::new(0.0425, 2, d(2034, 11, 15), DayCount::Act365F).unwrap()
    );
    assert_eq!(c.clean_price, Some(98.75));
    assert_eq!(c.margin, Some(Margin::Haircut(0.02)));
    assert_eq!(
        m.collateral(DBR).unwrap().margin,
        Some(Margin::InitialMargin(1.02))
    );

    // Special: GC − 40bp; GC bond: GC; unknown ISIN: error, not GC.
    assert!((m.repo_rate(UST, 30).unwrap() - (0.0368 - 0.0040)).abs() < 1e-15);
    assert_eq!(m.repo_rate(UST_GC, 30).unwrap(), 0.0368);
    assert!(matches!(
        m.repo_rate("XS0000000000", 30),
        Err(RepoError::MissingMarketData(_))
    ));
    assert!(m.gc_curve("GBP").is_err());
}

/// A trade built from the snapshot equals the hand-built trade of
/// examples/repo_report.rs, so every downstream analytic agrees too.
#[test]
fn snapshot_trade_matches_hand_built_trade() {
    let m = market();
    let asof = d(2026, 11, 9);
    let end = d(2026, 12, 9);
    let from_snapshot = m
        .repo_trade(UST, Direction::Repo, 50_000_000.0, Some(end))
        .unwrap();

    let gc = m.gc_curve("USD").unwrap();
    let by_hand = RepoTrade {
        direction: Direction::Repo,
        collateral: Bond::new(0.0425, 2, d(2034, 11, 15), DayCount::Act365F).unwrap(),
        nominal: 50_000_000.0,
        clean_price: 98.75,
        start: asof,
        end: Some(end),
        repo_rate: gc.rate(30) - 0.0040,
        day_count: DayCount::Act360,
        margin: Margin::Haircut(0.02),
    };
    assert_eq!(from_snapshot, by_hand);
    assert_eq!(
        from_snapshot.repurchase_price().unwrap(),
        by_hand.repurchase_price().unwrap()
    );

    // Open repo: ON rate.
    let open = m
        .repo_trade(UST_GC, Direction::ReverseRepo, 1e6, None)
        .unwrap();
    assert_eq!(open.repo_rate, 0.0365);
    // EUR bond: EUR curve and initial margin.
    let eur = m.repo_trade(DBR, Direction::Repo, 1e6, Some(end)).unwrap();
    assert_eq!(eur.repo_rate, 0.0195);
    assert_eq!(eur.margin, Margin::InitialMargin(1.02));
}

#[test]
fn fixings_compound_like_the_core_function() {
    let m = market();
    let sofr = m.fixings("SOFR").unwrap();
    assert_eq!(
        (sofr.currency.as_str(), sofr.day_count),
        ("USD", DayCount::Act360)
    );
    assert!(sofr.fixings.last().unwrap().0 < m.asof);
    let (start, end) = (d(2026, 10, 9), d(2026, 11, 9));
    let r = m.compounded("SOFR", start, end).unwrap();
    assert_eq!(
        r,
        compounded_overnight(&sofr.fixings, start, end, DayCount::Act360).unwrap()
    );
    // Compounding sits between the window's fixings (plus a little convexity).
    let window: Vec<f64> = sofr
        .fixings
        .iter()
        .filter(|(day, _)| *day >= start && *day < end)
        .map(|&(_, rate)| rate)
        .collect();
    let lo = window.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = window.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    assert!(
        window.len() >= 20 && r > lo && r < hi + 1e-4,
        "{r} vs [{lo}, {hi}]"
    );
    assert!(m.compounded("SONIA", start, end).is_err());
}

#[test]
fn snapshot_round_trips_through_rust() {
    let text = golden("repo_snapshot.golden.json");
    let snap = RepoSnapshot::from_json(&text).unwrap();
    assert_eq!(snap.schema_version, SCHEMA_VERSION);
    // Typed comparison: Go writes `100`, Rust `100.0`; both decoders accept either.
    let back = RepoSnapshot::from_json(&snap.to_json().unwrap()).unwrap();
    assert_eq!(back, snap);
    // Same keys at every level as the Go output (no renamed or dropped fields).
    fn keys(v: &Value, path: String, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => m.iter().for_each(|(k, x)| {
                out.push(format!("{path}/{k}"));
                keys(x, format!("{path}/{k}"), out)
            }),
            Value::Array(a) => a.iter().for_each(|x| keys(x, format!("{path}[]"), out)),
            _ => {}
        }
    }
    let (mut a, mut b) = (vec![], vec![]);
    keys(&serde_json::from_str(&text).unwrap(), String::new(), &mut a);
    keys(
        &serde_json::from_str(&snap.to_json().unwrap()).unwrap(),
        String::new(),
        &mut b,
    );
    assert_eq!(a, b);
}

/// Applies `mutate` to the golden JSON and expects the load to fail with an
/// error containing `want`.
fn rejects(mutate: impl FnOnce(&mut Value), want: &str) {
    let mut v: Value = serde_json::from_str(&golden("repo_snapshot.golden.json")).unwrap();
    mutate(&mut v);
    match RepoMarket::from_json(&v.to_string()) {
        Ok(_) => panic!("snapshot accepted; expected error containing {want:?}"),
        Err(e) => assert!(e.to_string().contains(want), "error {e:?} lacks {want:?}"),
    }
}

#[test]
fn invalid_snapshots_are_rejected() {
    rejects(|v| v["schema_version"] = json!(2), "schema_version 2");
    rejects(|v| v["threshold"] = json!(50_000), "unknown field");
    rejects(|v| v["bonds"][UST]["isin"] = json!(UST), "unknown field");
    rejects(
        |v| v["gc_curves"]["USD"]["pillars"][2]["days"] = json!(31),
        "pillar 1M has 31 days, but 30",
    );
    rejects(
        |v| v["gc_curves"]["USD"]["pillars"][0]["rate"] = json!(3.65),
        "not a decimal rate",
    );
    rejects(
        |v| v["gc_curves"]["USD"]["day_count"] = json!("ACT/ACT"),
        "unknown variant",
    );
    rejects(
        |v| {
            let p = v["gc_curves"]["USD"]["pillars"].as_array_mut().unwrap();
            p.swap(1, 2);
        },
        "strictly increasing",
    );
    rejects(
        |v| v["haircuts"][UST] = json!({"kind": "haircut", "value": 1.0}),
        "haircut 1 not in [0, 1)",
    );
    rejects(
        |v| v["haircuts"][UST] = json!({"kind": "pct", "value": 0.02}),
        "unknown variant",
    );
    rejects(
        |v| v["prices"]["XS0000000000"] = json!({"clean": 100.0}),
        "no bond static",
    );
    rejects(
        |v| {
            v["gc_curves"].as_object_mut().unwrap().remove("USD");
        },
        "no GC curve for currency USD",
    );
    rejects(
        |v| {
            v["fixings"]["SOFR"]["fixings"]
                .as_array_mut()
                .unwrap()
                .push(json!({"date": "2026-11-10", "rate": 0.0365}));
        },
        "after as-of",
    );
    rejects(
        |v| {
            let f = v["fixings"]["SOFR"]["fixings"].as_array_mut().unwrap();
            f.swap(0, 1);
        },
        "strictly increasing",
    );
    rejects(
        |v| v["bonds"][UST_GC]["frequency"] = json!(3),
        "frequency 3",
    );
    rejects(
        |v| v["bonds"][UST_GC]["maturity"] = json!("2026-11-09"),
        "matured",
    );
    rejects(
        |v| v["asof"] = json!("09/11/2026"),
        "input contains invalid characters",
    );
}

#[test]
fn missing_quotes_surface_as_errors() {
    let mut v: Value = serde_json::from_str(&golden("repo_snapshot.golden.json")).unwrap();
    v["prices"].as_object_mut().unwrap().remove(UST);
    v["haircuts"].as_object_mut().unwrap().remove(UST_GC);
    let m = RepoMarket::from_json(&v.to_string()).unwrap();
    let end = Some(d(2026, 12, 9));
    let e = m.repo_trade(UST, Direction::Repo, 1e6, end).unwrap_err();
    assert_eq!(
        e,
        RepoError::MissingMarketData(format!("clean price for {UST}"))
    );
    let e = m.repo_trade(UST_GC, Direction::Repo, 1e6, end).unwrap_err();
    assert_eq!(
        e,
        RepoError::MissingMarketData(format!("haircut for {UST_GC}"))
    );
}
