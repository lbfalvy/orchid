mod numeric;
pub mod operators;
pub use numeric::Numeric;

use crate::project::{fnlib_loader, Loader};

pub fn num() -> impl Loader {
  fnlib_loader(vec![
    ("add", Box::new(operators::add::Add2)),
    ("subtract", Box::new(operators::subtract::Subtract2)),
    ("multiply", Box::new(operators::multiply::Multiply2)),
    ("divide", Box::new(operators::divide::Divide2)),
    ("remainder", Box::new(operators::remainder::Remainder2))
  ])
}