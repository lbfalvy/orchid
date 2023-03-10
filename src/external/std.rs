use std::collections::HashMap;

use crate::project::{map_loader, Loader};

use super::bool::bool;
use super::cpsio::cpsio;
use super::conv::conv;
use super::str::str;
use super::num::num;

pub fn std() -> impl Loader {
  map_loader(HashMap::from([
    ("cpsio", cpsio().boxed()),
    ("conv", conv().boxed()),
    ("bool", bool().boxed()),
    ("str", str().boxed()),
    ("num", num().boxed()),
  ]))
}