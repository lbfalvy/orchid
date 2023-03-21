mod concatenate;
mod cls2str;
mod char_at;
pub use cls2str::cls2str;
use crate::project::{Loader, extlib_loader};

pub fn str() -> impl Loader {
  extlib_loader(vec![
    ("concatenate", Box::new(concatenate::Concatenate2))
  ])
}