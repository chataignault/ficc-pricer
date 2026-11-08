use std::fmt;

use chrono::NaiveDate;

/// Errors raised by the repo analytics.
#[derive(Debug, Clone, PartialEq)]
pub enum RepoError {
    /// An open repo was asked for a quantity that needs a maturity date.
    MissingEndDate,
    /// `end` is not strictly after `start`.
    InvalidDates { start: NaiveDate, end: NaiveDate },
    /// A numeric input is out of its admissible range.
    InvalidInput(String),
    /// An iterative solver failed to converge.
    NoConvergence,
    /// A curve was built without pillars.
    EmptyCurve,
    /// Curve pillars are not strictly increasing in maturity.
    UnsortedPillars,
    /// A tenor string could not be parsed (expected `ON`, `TN`, `1W`, `3M`, `1Y`...).
    UnknownTenor(String),
    /// Overnight fixings do not cover the requested period.
    MissingFixing(NaiveDate),
}

impl fmt::Display for RepoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepoError::MissingEndDate => write!(f, "open repo has no end date"),
            RepoError::InvalidDates { start, end } => {
                write!(f, "end date {end} must be after start date {start}")
            }
            RepoError::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            RepoError::NoConvergence => write!(f, "solver did not converge"),
            RepoError::EmptyCurve => write!(f, "curve has no pillars"),
            RepoError::UnsortedPillars => write!(f, "curve pillars must be strictly increasing"),
            RepoError::UnknownTenor(t) => write!(f, "unknown tenor: {t}"),
            RepoError::MissingFixing(d) => write!(f, "no overnight fixing on or before {d}"),
        }
    }
}

impl std::error::Error for RepoError {}

pub type Result<T> = std::result::Result<T, RepoError>;

/// Fails unless `end > start`.
pub(crate) fn check_dates(start: NaiveDate, end: NaiveDate) -> Result<()> {
    if end > start {
        Ok(())
    } else {
        Err(RepoError::InvalidDates { start, end })
    }
}
