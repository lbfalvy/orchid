use std::fmt::Display;
use std::ops::Range;
use std::rc::Rc;

use itertools::Itertools;

/// A location in a file, identifies a sequence of suspect characters for any
/// error. Meaningful within the context of a project.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Location {
  /// Location information lost or code generated on the fly
  Unknown,
  /// Only the file is known
  File(Rc<Vec<String>>),
  /// Character slice of the code
  Range {
    /// Argument to the file loading callback that produced this code
    file: Rc<Vec<String>>,
    /// Index of the unicode code points associated with the code
    range: Range<usize>,
  },
}

impl Location {
  /// Range, if known. If the range is known, the file is always known
  pub fn range(&self) -> Option<Range<usize>> {
    if let Self::Range { range, .. } = self {
      Some(range.clone())
    } else {
      None
    }
  }

  /// File, if known
  pub fn file(&self) -> Option<Rc<Vec<String>>> {
    if let Self::File(file) | Self::Range { file, .. } = self {
      Some(file.clone())
    } else {
      None
    }
  }

  /// If the two locations are ranges in the same file, connect them.
  /// Otherwise choose the more accurate, preferring lhs if equal.
  pub fn to(self, other: Self) -> Self {
    match self {
      Location::Unknown => other,
      Location::File(f) => match other {
        Location::Range { .. } => other,
        _ => Location::File(f),
      },
      Location::Range { file: f1, range: r1 } => Location::Range {
        range: match other {
          Location::Range { file: f2, range: r2 } if f1 == f2 =>
            r1.start..r2.end,
          _ => r1,
        },
        file: f1,
      },
    }
  }

  /// Choose one of the two locations, preferring better accuracy, or lhs if
  /// equal
  pub fn or(self, alt: Self) -> Self {
    match (&self, &alt) {
      (Self::Unknown, _) => alt,
      (Self::File(_), Self::Range { .. }) => alt,
      _ => self,
    }
  }
}

impl Display for Location {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Unknown => write!(f, "unknown"),
      Self::File(file) => write!(f, "{}.orc", file.iter().join("/")),
      Self::Range { file, range } => write!(
        f,
        "{}.orc:{}..{}",
        file.iter().join("/"),
        range.start,
        range.end
      ),
    }
  }
}
