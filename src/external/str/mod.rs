mod concatenate;
mod cls2str;
mod char_at;
pub use cls2str::cls2str;
use crate::project::{Loader, fnlib_loader};

pub fn str() -> impl Loader {
  fnlib_loader(vec![
    ("concatenate", Box::new(concatenate::Concatenate2))
  ])
}