use std::{fmt, error::Error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleError {
    BadState(String),
    ScalarVecMismatch(String)
}

impl fmt::Display for RuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadState(key) => write!(f, "Key {:?} not in match pattern", key),
            Self::ScalarVecMismatch(key) =>
                write!(f, "Key {:?} used inconsistently with and without ellipsis", key)
        }
    }
}
impl Error for RuleError {}