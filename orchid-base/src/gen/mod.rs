//! Abstractions and primitives for defining Orchid code in compile-time Rust
//! constants. This is used both to generate glue code such as function call
//! expressions at runtime and to define completely static intrinsics and
//! constants accessible to usercode.
pub mod tpl;
pub mod traits;
pub mod tree;
pub mod impls;
