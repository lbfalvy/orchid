use crate::project::{Loader, fnlib_loader};

mod print;
mod readline;

pub fn cpsio() -> impl Loader {
  fnlib_loader(vec![
    ("print", Box::new(print::Print2)),
    ("readline", Box::new(readline::Readln2))
  ])
}