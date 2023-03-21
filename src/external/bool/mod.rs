mod equals;
mod boolean;
mod ifthenelse;
pub use boolean::Boolean;

use crate::project::{Loader, extlib_loader};

pub fn bool() -> impl Loader {
  extlib_loader(vec![
    ("ifthenelse", Box::new(ifthenelse::IfThenElse1)),
    ("equals", Box::new(equals::Equals2))
  ])
}