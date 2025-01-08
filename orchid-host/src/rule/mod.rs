//! Optimized form of macro pattern that can be quickly tested against the AST.
//!
//! # Construction
//!
//! convert pattern into hierarchy of plain, scan, middle
//! - plain: accept any sequence or any non-empty sequence
//! - scan: a single scalar pattern moves LTR or RTL, submatchers on either
//!   side
//! - middle: two scalar patterns walk over all permutations of matches
//!   while getting progressively closer to each other
//!
//! # Application
//!
//! walk over the current matcher's valid options and poll the submatchers
//! for each of them

mod any_match;
mod build;
mod scal_match;
pub mod shared;
mod vec_match;
pub mod state;
mod vec_attrs;
pub mod matcher;
// pub mod matcher;