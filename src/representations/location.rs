use std::{ops::Range, rc::Rc, fmt::Display};

use itertools::Itertools;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Location {
  Unknown,
  File(Rc<Vec<String>>),
  Range{
    file: Rc<Vec<String>>,
    range: Range<usize>,
  }
}

impl Location {
  pub fn range(&self) -> Option<Range<usize>> {
    if let Self::Range{ range, .. } = self {
      Some(range.clone())
    } else { None }
  }

  pub fn file(&self) -> Option<Rc<Vec<String>>> {
    if let Self::File(file) | Self::Range { file, .. } = self {
      Some(file.clone())
    } else { None }
  }
}

impl Display for Location {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Unknown => write!(f, "unknown"),
      Self::File(file) => write!(f, "{}.orc", file.iter().join("/")),
      Self::Range{ file, range } => write!(f,
        "{}.orc:{}..{}",
        file.iter().join("/"), range.start, range.end
      )
    }
  }
}
