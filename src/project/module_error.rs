use thiserror::Error;

use crate::parse::ParseError;

use super::name_resolver::ResolutionError;

#[derive(Error, Debug, Clone)]
pub enum ModuleError<ELoad> where ELoad: Clone {
    #[error("Resolution cycle")]
    ResolutionCycle,
    #[error("File not found: {0}")]
    Load(ELoad),
    #[error("Failed to parse: {0:?}")]
    Syntax(ParseError),
    #[error("Not a module")]
    None
}

impl<T> From<ParseError> for ModuleError<T> where T: Clone {
    fn from(pars: ParseError) -> Self { Self::Syntax(pars) }
}

impl<T> From<ResolutionError<ModuleError<T>>> for ModuleError<T> where T: Clone {
    fn from(res: ResolutionError<ModuleError<T>>) -> Self {
        match res {
            ResolutionError::Cycle(_) => ModuleError::ResolutionCycle,
            ResolutionError::NoModule(_) => ModuleError::None,
            ResolutionError::Delegate(d) => d
        }
    }
}