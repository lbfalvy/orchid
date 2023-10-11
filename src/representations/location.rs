use std::fmt::{Debug, Display};
use std::ops::Range;
use std::sync::Arc;

use itertools::Itertools;

use crate::VName;

/// A location in a file, identifies a sequence of suspect characters for any
/// error. Meaningful within the context of a project.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Location {
  /// Location information lost or code generated on the fly
  Unknown,
  /// Only the file is known
  File(Arc<VName>),
  /// Character slice of the code
  Range {
    /// Argument to the file loading callback that produced this code
    file: Arc<VName>,
    /// Index of the unicode code points associated with the code
    range: Range<usize>,
    /// The full source code as received by the parser
    source: Arc<String>,
  },
}

impl Location {
  /// Range, if known. If the range is known, the file is always known
  #[must_use]
  pub fn range(&self) -> Option<Range<usize>> {
    if let Self::Range { range, .. } = self {
      Some(range.clone())
    } else {
      None
    }
  }

  /// File, if known
  #[must_use]
  pub fn file(&self) -> Option<Arc<VName>> {
    if let Self::File(file) | Self::Range { file, .. } = self {
      Some(file.clone())
    } else {
      None
    }
  }

  /// Associated source code, if known
  #[must_use]
  pub fn source(&self) -> Option<Arc<String>> {
    if let Self::Range { source, .. } = self {
      Some(source.clone())
    } else {
      None
    }
  }

  /// If the two locations are ranges in the same file, connect them.
  /// Otherwise choose the more accurate, preferring lhs if equal.
  #[must_use]
  pub fn to(self, other: Self) -> Self {
    match self {
      Location::Unknown => other,
      Location::File(f) => match other {
        Location::Range { .. } => other,
        _ => Location::File(f),
      },
      Location::Range { file, range: r1, source } => {
        let range = match other {
          Location::Range { file: f2, range: r2, .. } if file == f2 =>
            r1.start..r2.end,
          _ => r1,
        };
        Location::Range { file, source, range }
      },
    }
  }

  /// Choose one of the two locations, preferring better accuracy, or lhs if
  /// equal
  #[must_use]
  pub fn or(self, alt: Self) -> Self {
    match (&self, &alt) {
      (Self::Unknown, _) => alt,
      (Self::File { .. }, Self::Range { .. }) => alt,
      _ => self,
    }
  }
}

impl Display for Location {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Unknown => write!(f, "unknown"),
      Self::File(file) => write!(f, "{}.orc", file.iter().join("/")),
      Self::Range { file, range, source } => {
        let (sl, sc) = pos2lc(source, range.start);
        let (el, ec) = pos2lc(source, range.end);
        write!(f, "{}.orc ", file.iter().join("/"))?;
        write!(f, "{sl}:{sc}")?;
        if el == sl {
          if sc + 1 == ec { Ok(()) } else { write!(f, "..{ec}") }
        } else {
          write!(f, "..{el}:{ec}")
        }
      },
    }
  }
}

impl Debug for Location {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "{self}")
  }
}

#[must_use]
fn pos2lc(s: &str, i: usize) -> (usize, usize) {
  s.chars().take(i).fold((1, 1), |(line, col), char| {
    if char == '\n' { (line + 1, 1) } else { (line, col + 1) }
  })
}
