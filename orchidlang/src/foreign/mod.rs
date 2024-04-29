//! Interaction with foreign code
//!
//! Structures and traits used in the exposure of external functions and values
//! to Orchid code
pub mod atom;
pub mod cps_box;
pub mod error;
pub mod fn_bridge;
pub mod inert;
pub mod process;
pub mod to_clause;
pub mod try_from_expr;
