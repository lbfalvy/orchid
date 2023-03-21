use crate::project::{Loader, extlib_loader};

mod print;
mod readline;

pub fn cpsio() -> impl Loader {
  extlib_loader(vec![
    ("print", Box::new(print::Print2)),
    ("readline", Box::new(readline::Readln2))
  ])
}