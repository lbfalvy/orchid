//! Constants exposed to usercode by the interpreter
mod assertion_error;
mod bool;
mod conv;
mod cpsio;
pub mod litconv;
mod mk_stl;
mod num;
mod runtime_error;
mod str;

pub use assertion_error::AssertionError;
pub use cpsio::{handle as handleIO, IO};
pub use mk_stl::mk_stl;
pub use runtime_error::RuntimeError;

pub use self::bool::Boolean;
pub use self::num::Numeric;
