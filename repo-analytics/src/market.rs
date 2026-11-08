//! Repo market snapshots exported by the Go `market-gateway`
//! (`market-gateway repo export`). Requires the `serde` feature.
//!
//! [`RepoSnapshot`] mirrors the JSON schema
//! (`market-gateway/internal/market/repo.go`) field for field.
//! [`RepoMarket`] is the validated form, built from the crate's own types:
//! [`RepoCurve`], [`SpecialsCurve`], [`Bond`] and [`Margin`].
//!
//! The loader re-checks everything the gateway checks, and also checks
//! that the gateway's tenor arithmetic (`days` of each pillar) agrees
//! with [`Tenor::end_date`].

use std::collections::{BTreeMap, HashMap};
use std::io::Read;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::bond::Bond;
use crate::curve::{compounded_overnight, RepoCurve, SpecialsCurve, Tenor};
use crate::daycount::DayCount;
use crate::error::{RepoError, Result};
use crate::trade::{Direction, Margin, RepoTrade};

/// Snapshot schema version understood by this crate.
pub const SCHEMA_VERSION: u32 = 1;

/// Sanity bound on rates and spreads; catches percent-vs-decimal mistakes.
const MAX_ABS_RATE: f64 = 1.0;

// ---------------------------------------------------------------------------
// Wire format
// ---------------------------------------------------------------------------

/// The JSON snapshot as written by the gateway. Unknown fields are rejected
/// so that schema drift between Go and Rust fails loudly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RepoSnapshot {
    pub schema_version: u32,
    pub asof: NaiveDate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_at: Option<String>,
    /// Keyed by currency.
    #[serde(default)]
    pub gc_curves: BTreeMap<String, CurveQuote>,
    /// Keyed by ISIN.
    #[serde(default)]
    pub specials: BTreeMap<String, SpecialQuote>,
    /// Keyed by index name (SOFR, ESTR...).
    #[serde(default)]
    pub fixings: BTreeMap<String, FixingSeriesQuote>,
    /// Keyed by ISIN.
    #[serde(default)]
    pub bonds: BTreeMap<String, BondQuote>,
    /// Keyed by ISIN.
    #[serde(default)]
    pub prices: BTreeMap<String, PriceQuote>,
    /// Keyed by ISIN.
    #[serde(default)]
    pub haircuts: BTreeMap<String, Margin>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurveQuote {
    pub day_count: DayCount,
    pub pillars: Vec<PillarQuote>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PillarQuote {
    pub tenor: String,
    pub days: i64,
    pub rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecialQuote {
    pub spread: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixingSeriesQuote {
    pub currency: String,
    pub day_count: DayCount,
    pub fixings: Vec<FixingQuote>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixingQuote {
    pub date: NaiveDate,
    pub rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BondQuote {
    pub currency: String,
    pub coupon_rate: f64,
    pub frequency: u32,
    pub maturity: NaiveDate,
    pub face: f64,
    pub day_count: DayCount,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriceQuote {
    /// Clean price per `face`.
    pub clean: f64,
}

impl RepoSnapshot {
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| RepoError::InvalidSnapshot(e.to_string()))
    }

    pub fn from_reader(reader: impl Read) -> Result<Self> {
        serde_json::from_reader(reader).map_err(|e| RepoError::InvalidSnapshot(e.to_string()))
    }

    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| RepoError::InvalidSnapshot(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Validated market
// ---------------------------------------------------------------------------

/// A collateral bond with its market data.
#[derive(Debug, Clone, PartialEq)]
pub struct Collateral {
    pub isin: String,
    pub currency: String,
    pub bond: Bond,
    /// Clean price per `bond.face` at the snapshot date, if quoted.
    pub clean_price: Option<f64>,
    /// Margining, if quoted.
    pub margin: Option<Margin>,
}

/// An overnight index fixing history.
#[derive(Debug, Clone, PartialEq)]
pub struct FixingSeries {
    pub currency: String,
    pub day_count: DayCount,
    pub fixings: Vec<(NaiveDate, f64)>,
}

/// A validated repo market, built from a [`RepoSnapshot`].
#[derive(Debug, Clone, PartialEq)]
pub struct RepoMarket {
    pub asof: NaiveDate,
    /// GC curve plus the specials of bonds in that currency, by currency.
    curves: HashMap<String, SpecialsCurve>,
    collateral: HashMap<String, Collateral>,
    fixings: HashMap<String, FixingSeries>,
}

fn invalid(msg: String) -> RepoError {
    RepoError::InvalidSnapshot(msg)
}

fn check_rate(what: &str, r: f64) -> Result<()> {
    if r.is_finite() && r.abs() < MAX_ABS_RATE {
        Ok(())
    } else {
        Err(invalid(format!(
            "{what} {r} is not a decimal rate (|r| < {MAX_ABS_RATE})"
        )))
    }
}

impl RepoMarket {
    pub fn from_json(json: &str) -> Result<Self> {
        Self::from_snapshot(RepoSnapshot::from_json(json)?)
    }

    pub fn from_reader(reader: impl Read) -> Result<Self> {
        Self::from_snapshot(RepoSnapshot::from_reader(reader)?)
    }

    /// Validates the snapshot and builds the market.
    pub fn from_snapshot(s: RepoSnapshot) -> Result<Self> {
        if s.schema_version != SCHEMA_VERSION {
            return Err(invalid(format!(
                "schema_version {}, this crate reads {SCHEMA_VERSION}",
                s.schema_version
            )));
        }
        let asof = s.asof;

        let mut curves = HashMap::new();
        for (ccy, c) in &s.gc_curves {
            let mut pillars = Vec::with_capacity(c.pillars.len());
            for p in &c.pillars {
                let tenor = Tenor::parse(&p.tenor)
                    .map_err(|e| invalid(format!("gc_curves[{ccy}]: {e}")))?;
                let days = (tenor.end_date(asof) - asof).num_days();
                if days != p.days {
                    return Err(invalid(format!(
                        "gc_curves[{ccy}]: pillar {} has {} days, but {} from {asof}",
                        p.tenor, p.days, days
                    )));
                }
                check_rate(&format!("gc_curves[{ccy}] {} rate", p.tenor), p.rate)?;
                pillars.push((p.days, p.rate));
            }
            let curve = RepoCurve::new(asof, c.day_count, pillars)
                .map_err(|e| invalid(format!("gc_curves[{ccy}]: {e}")))?;
            curves.insert(ccy.clone(), SpecialsCurve::new(curve));
        }

        let mut collateral = HashMap::new();
        for (isin, b) in &s.bonds {
            if b.currency.is_empty() {
                return Err(invalid(format!("bonds[{isin}]: missing currency")));
            }
            if !(b.face.is_finite() && b.face > 0.0) {
                return Err(invalid(format!(
                    "bonds[{isin}]: face {} must be positive",
                    b.face
                )));
            }
            check_rate(&format!("bonds[{isin}] coupon"), b.coupon_rate)?;
            if b.maturity <= asof {
                return Err(invalid(format!("bonds[{isin}]: matured on {}", b.maturity)));
            }
            let mut bond = Bond::new(b.coupon_rate, b.frequency, b.maturity, b.day_count)
                .map_err(|e| invalid(format!("bonds[{isin}]: {e}")))?;
            bond.face = b.face;
            collateral.insert(
                isin.clone(),
                Collateral {
                    isin: isin.clone(),
                    currency: b.currency.clone(),
                    bond,
                    clean_price: None,
                    margin: None,
                },
            );
        }

        for (isin, sp) in &s.specials {
            let ccy = &collateral
                .get(isin)
                .ok_or_else(|| invalid(format!("specials[{isin}]: no bond static for this ISIN")))?
                .currency;
            check_rate(&format!("specials[{isin}] spread"), sp.spread)?;
            curves
                .get_mut(ccy)
                .ok_or_else(|| {
                    invalid(format!("specials[{isin}]: no GC curve for currency {ccy}"))
                })?
                .set_spread(isin.clone(), sp.spread);
        }

        for (isin, p) in &s.prices {
            let c = collateral
                .get_mut(isin)
                .ok_or_else(|| invalid(format!("prices[{isin}]: no bond static for this ISIN")))?;
            if !(p.clean.is_finite() && p.clean > 0.0) {
                return Err(invalid(format!(
                    "prices[{isin}]: clean price {} must be positive",
                    p.clean
                )));
            }
            c.clean_price = Some(p.clean);
        }

        for (isin, m) in &s.haircuts {
            let c = collateral.get_mut(isin).ok_or_else(|| {
                invalid(format!("haircuts[{isin}]: no bond static for this ISIN"))
            })?;
            m.validate()
                .map_err(|e| invalid(format!("haircuts[{isin}]: {e}")))?;
            c.margin = Some(*m);
        }

        let mut fixings = HashMap::new();
        for (index, f) in &s.fixings {
            if f.currency.is_empty() {
                return Err(invalid(format!("fixings[{index}]: missing currency")));
            }
            let mut series = Vec::with_capacity(f.fixings.len());
            for x in &f.fixings {
                if let Some(&(prev, _)) = series.last() {
                    if x.date <= prev {
                        return Err(invalid(format!(
                            "fixings[{index}]: dates must be strictly increasing ({})",
                            x.date
                        )));
                    }
                }
                if x.date > asof {
                    return Err(invalid(format!(
                        "fixings[{index}]: fixing {} is after as-of {asof}",
                        x.date
                    )));
                }
                check_rate(&format!("fixings[{index}] {}", x.date), x.rate)?;
                series.push((x.date, x.rate));
            }
            fixings.insert(
                index.clone(),
                FixingSeries {
                    currency: f.currency.clone(),
                    day_count: f.day_count,
                    fixings: series,
                },
            );
        }

        Ok(RepoMarket {
            asof,
            curves,
            collateral,
            fixings,
        })
    }

    /// Currencies with a GC curve, sorted.
    pub fn currencies(&self) -> Vec<&str> {
        let mut v: Vec<_> = self.curves.keys().map(String::as_str).collect();
        v.sort_unstable();
        v
    }

    /// ISINs with bond static, sorted.
    pub fn isins(&self) -> Vec<&str> {
        let mut v: Vec<_> = self.collateral.keys().map(String::as_str).collect();
        v.sort_unstable();
        v
    }

    /// GC curve plus specials for a currency.
    pub fn specials(&self, currency: &str) -> Result<&SpecialsCurve> {
        self.curves
            .get(currency)
            .ok_or_else(|| RepoError::MissingMarketData(format!("GC curve for {currency}")))
    }

    pub fn gc_curve(&self, currency: &str) -> Result<&RepoCurve> {
        Ok(&self.specials(currency)?.gc)
    }

    pub fn collateral(&self, isin: &str) -> Result<&Collateral> {
        self.collateral
            .get(isin)
            .ok_or_else(|| RepoError::MissingMarketData(format!("bond static for {isin}")))
    }

    /// Repo rate for `isin` over `days`: GC in the bond's currency minus
    /// its specialness. A known bond with no special quote trades at GC; an
    /// unknown ISIN is an error, not a silent GC fallback.
    pub fn repo_rate(&self, isin: &str, days: i64) -> Result<f64> {
        let c = self.collateral(isin)?;
        Ok(self.specials(&c.currency)?.special_rate(isin, days))
    }

    pub fn fixings(&self, index: &str) -> Result<&FixingSeries> {
        self.fixings
            .get(index)
            .ok_or_else(|| RepoError::MissingMarketData(format!("fixings for {index}")))
    }

    /// Compounded overnight rate of `index` over `[start, end)`, using the
    /// index day count.
    pub fn compounded(&self, index: &str, start: NaiveDate, end: NaiveDate) -> Result<f64> {
        let f = self.fixings(index)?;
        compounded_overnight(&f.fixings, start, end, f.day_count)
    }

    /// A repo on `isin` starting at the snapshot date, priced off the
    /// snapshot: clean price, haircut, and the GC-minus-special rate for the
    /// term (ON rate for an open repo). The repo day count is the GC curve's.
    pub fn repo_trade(
        &self,
        isin: &str,
        direction: Direction,
        nominal: f64,
        end: Option<NaiveDate>,
    ) -> Result<RepoTrade> {
        let c = self.collateral(isin)?;
        let clean_price = c
            .clean_price
            .ok_or_else(|| RepoError::MissingMarketData(format!("clean price for {isin}")))?;
        let margin = c
            .margin
            .ok_or_else(|| RepoError::MissingMarketData(format!("haircut for {isin}")))?;
        let days = match end {
            Some(e) => (e - self.asof).num_days(),
            None => 1,
        };
        let trade = RepoTrade {
            direction,
            collateral: c.bond.clone(),
            nominal,
            clean_price,
            start: self.asof,
            end,
            repo_rate: self.repo_rate(isin, days)?,
            day_count: self.gc_curve(&c.currency)?.day_count,
            margin,
        };
        trade.validate()?;
        Ok(trade)
    }
}
