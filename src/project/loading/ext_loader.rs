use lasso::Spur;

use crate::representations::sourcefile::FileEntry;

use super::{Loader, Loaded, LoadingError};

pub fn ext_loader<'a, T, F>(
  data: Vec<FileEntry>,
  mut submods: Vec<(&'static str, T)>,
  intern: &'a F
) -> impl Loader + 'a
where
  T: Loader + 'a,
  F: Fn(&str) -> Spur {
  move |path: &[&str]| {
    let (step, rest) = match path.split_first() {
      None => return Ok(Loaded::AST(
        data.iter().cloned().chain(
          submods.iter().map(|(s, _)| FileEntry::LazyModule(intern(s)))
        ).collect()
      )),
      Some(t) => t
    };
    if let Some((_, l)) = submods.iter_mut().find(|(s, l)| s == step) {
      l.load(rest)
    } else {
      let errtyp = if rest.is_empty() {
        LoadingError::UnknownNode
      } else {LoadingError::Missing};
      Err(errtyp(step.to_string()))
    }
  }
}