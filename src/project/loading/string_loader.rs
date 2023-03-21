use super::{Loader, Loaded};

pub fn string_loader<'a>(data: &'a str) -> impl Loader + 'a {
  move |_: &[&str]| Ok(Loaded::Source(data.to_string()))
}