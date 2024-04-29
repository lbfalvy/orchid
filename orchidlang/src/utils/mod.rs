//! Utilities that don't necessarily have a well-defined role in the
//! problem-domain of Orchid but are rather designed to fulfill abstract
//! project-domain tasks.
//!
//! An unreferenced util should be either moved out to a package or deleted

pub(crate) mod boxed_iter;
pub(crate) mod clonable_iter;
pub mod combine;
pub mod ddispatch;
pub(crate) mod get_or;
pub(crate) mod iter_find;
pub mod join;
pub mod pure_seq;
pub mod sequence;
pub mod side;
pub mod string_from_charset;
pub mod take_with_output;
pub(crate) mod unwrap_or;
