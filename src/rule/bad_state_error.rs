use std::{fmt, error::Error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BadState(Vec<String>);

impl fmt::Display for BadState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "The following key(s) weren't produced by the matching pattern: {:?}", self.0)
    }
}
impl Error for BadState {}