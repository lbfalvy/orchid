use crate::project::{fnlib_loader, Loader};

mod to_string;
mod parse_float;
mod parse_uint;

pub fn conv() -> impl Loader {
  fnlib_loader(vec![
    ("parse_float", Box::new(parse_float::ParseFloat1)),
    ("parse_uint", Box::new(parse_uint::ParseUint1)),
    ("to_string", Box::new(to_string::ToString1))
  ])
}