use std::collections::HashMap;

use super::{Loader, LoadingError, Loaded};

pub fn map_loader<'a, T: Loader + 'a>(mut map: HashMap<&'a str, T>) -> impl Loader + 'a {
  move |path: &[&str]| {
    let (key, subpath) = if let Some(sf) = path.split_first() {sf}
    else {return Ok(Loaded::Source(map.keys().cloned().collect()))};
    let sub = if let Some(sub) = map.get_mut(key.to_string().as_str()) {sub}
    else {return Err(
      if subpath.len() == 0 {LoadingError::UnknownNode(path.join("::"))}
      else {LoadingError::Missing(path.join("::"))}
    )};
    sub.load(subpath)
  }
}