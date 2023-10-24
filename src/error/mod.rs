//! Various errors the pipeline can produce
mod assertion_error;
mod conflicting_roles;
mod import_all;
mod no_targets;
mod not_exported;
mod project_error;
mod runtime_error;
mod too_many_supers;
mod unexpected_directory;
mod visibility_mismatch;

pub use assertion_error::AssertionError;
pub use conflicting_roles::ConflictingRoles;
pub use import_all::ImportAll;
pub use no_targets::NoTargets;
pub use not_exported::NotExported;
pub use project_error::{ErrorPosition, ProjectError, ProjectResult};
pub use runtime_error::RuntimeError;
pub use too_many_supers::TooManySupers;
pub use unexpected_directory::UnexpectedDirectory;
pub use visibility_mismatch::VisibilityMismatch;
