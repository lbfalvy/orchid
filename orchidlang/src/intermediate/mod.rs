//! Intermediate representation. Currently just an indirection between
//! [super::parse::parsed] and [super::interpreter::nort], in the future
//! hopefully a common point for alternate encodings, optimizations and targets.

pub mod ast_to_ir;
pub mod ir;
pub mod ir_to_nort;
