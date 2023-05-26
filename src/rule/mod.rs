//! Substitution rule processing
mod matcher;
mod matcher_vectree;
mod prepare_rule;
mod repository;
mod rule_error;
mod state;
mod update_first_seq;
mod vec_attrs;

pub use matcher::Matcher;
pub use matcher_vectree::VectreeMatcher;
pub use repository::{Repo, Repository};
pub use rule_error::RuleError;
