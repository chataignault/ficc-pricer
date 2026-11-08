//! Standalone repo market analytics for the FICC pricer.
//!
//! - [`trade`]: classic repo economics (cash amount, interest, repurchase price)
//! - [`carry`]: bond forward price via repo, implied repo rate, carry
//! - [`risk`]: mark-to-market, margin calls, DV01s, specialness
//! - [`curve`]: GC repo curve, forward repo, compounded overnight, specials
//! - [`bond`]: fixed-coupon collateral valuation

pub mod bond;
pub mod carry;
pub mod curve;
pub mod daycount;
pub mod error;
pub mod risk;
pub mod trade;

pub use bond::Bond;
pub use curve::{compounded_overnight, RepoCurve, SpecialsCurve, Tenor};
pub use daycount::DayCount;
pub use error::{RepoError, Result};
pub use trade::{Direction, Margin, RepoTrade};
