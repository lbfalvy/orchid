//! IR is an abstract representation of Orchid expressions that's impractical
//! for all purposes except converting to and from other representations. Future
//! innovations in the processing and execution of code will likely operate on
//! this representation.

use std::fmt;
use std::rc::Rc;

use crate::foreign::atom::AtomGenerator;
use crate::location::{CodeLocation, SourceRange};
use crate::name::Sym;
use crate::utils::string_from_charset::string_from_charset;

/// Indicates whether either side needs to be wrapped. Syntax whose end is
/// ambiguous on that side must use parentheses, or forward the flag
#[derive(PartialEq, Eq, Clone, Copy)]
struct Wrap(bool, bool);

/// Code element with associated metadata
#[derive(Clone)]
pub struct Expr {
  /// Code element
  pub value: Clause,
  /// Location metadata
  pub location: CodeLocation,
}

impl Expr {
  /// Create an IR expression
  pub fn new(value: Clause, location: SourceRange, module: Sym) -> Self {
    Self { value, location: CodeLocation::new_src(location, module) }
  }

  fn deep_fmt(&self, f: &mut fmt::Formatter<'_>, depth: usize, tr: Wrap) -> fmt::Result {
    let Expr { value, .. } = self;
    value.deep_fmt(f, depth, tr)?;
    Ok(())
  }
}

impl fmt::Debug for Expr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.deep_fmt(f, 0, Wrap(false, false))
  }
}

/// Semantic code element
#[derive(Clone)]
pub enum Clause {
  /// Function call expression
  Apply(Rc<Expr>, Rc<Expr>),
  /// Function expression
  Lambda(Rc<Expr>),
  /// Reference to an external constant
  Constant(Sym),
  /// Reference to a function argument
  LambdaArg(usize),
  /// An opaque non-callable value, eg. a file handle
  Atom(AtomGenerator),
}

const ARGNAME_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz";

fn parametric_fmt(
  f: &mut fmt::Formatter<'_>,
  depth: usize,
  prefix: &str,
  body: &Expr,
  wrap_right: bool,
) -> fmt::Result {
  if wrap_right {
    write!(f, "(")?;
  }
  f.write_str(prefix)?;
  f.write_str(&string_from_charset(depth as u64, ARGNAME_CHARSET))?;
  f.write_str(".")?;
  body.deep_fmt(f, depth + 1, Wrap(false, false))?;
  if wrap_right {
    write!(f, ")")?;
  }
  Ok(())
}

impl Clause {
  fn deep_fmt(&self, f: &mut fmt::Formatter<'_>, depth: usize, Wrap(wl, wr): Wrap) -> fmt::Result {
    match self {
      Self::Atom(a) => write!(f, "{a:?}"),
      Self::Lambda(body) => parametric_fmt(f, depth, "\\", body, wr),
      Self::LambdaArg(skip) => {
        let lambda_depth = (depth - skip - 1).try_into().unwrap();
        f.write_str(&string_from_charset(lambda_depth, ARGNAME_CHARSET))
      },
      Self::Apply(func, x) => {
        if wl {
          write!(f, "(")?;
        }
        func.deep_fmt(f, depth, Wrap(false, true))?;
        write!(f, " ")?;
        x.deep_fmt(f, depth, Wrap(true, wr && !wl))?;
        if wl {
          write!(f, ")")?;
        }
        Ok(())
      },
      Self::Constant(token) => write!(f, "{token}"),
    }
  }
}

impl fmt::Debug for Clause {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    self.deep_fmt(f, 0, Wrap(false, false))
  }
}
