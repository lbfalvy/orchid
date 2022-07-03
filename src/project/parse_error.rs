use chumsky::prelude::Simple;
use thiserror::Error;

use super::name_resolver::ResolutionError;

#[derive(Error, Debug, Clone)]
pub enum ParseError<ELoad> where ELoad: Clone {
    #[error("Resolution cycle")]
    ResolutionCycle,
    #[error("File not found: {0}")]
    Load(ELoad),
    #[error("Failed to parse: {0:?}")]
    Syntax(Vec<Simple<char>>),
    #[error("Not a module")]
    None
}

impl<T> From<Vec<Simple<char>>> for ParseError<T> where T: Clone {
    fn from(simp: Vec<Simple<char>>) -> Self { Self::Syntax(simp) }
}

impl<T> From<ResolutionError<ParseError<T>>> for ParseError<T> where T: Clone {
    fn from(res: ResolutionError<ParseError<T>>) -> Self {
        match res {
            ResolutionError::Cycle(_) => ParseError::ResolutionCycle,
            ResolutionError::NoModule(_) => ParseError::None,
            ResolutionError::Delegate(d) => d
        }
    }
}