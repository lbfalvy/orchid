use std::fmt::Write;
use std::rc::Rc;

use crate::ast::Expr;
use crate::rule::{matcher::Matcher, state::State};
use crate::unwrap_or;
use crate::utils::{Side, print_nname};
use crate::interner::{Token, InternedDisplay, Interner};
use crate::representations::Primitive;

use super::{build::mk_matcher, any_match::any_match};

pub enum ScalMatcher {
  P(Primitive),
  Name(Token<Vec<Token<String>>>),
  S(char, Box<AnyMatcher>),
  Lambda(Box<ScalMatcher>, Box<AnyMatcher>),
  Placeh(Token<String>),
}

pub enum VecMatcher {
  Placeh{
    key: Token<String>,
    nonzero: bool
  },
  Scan{
    left: Box<VecMatcher>,
    sep: Vec<ScalMatcher>,
    right: Box<VecMatcher>,
    /// The separator traverses the sequence towards this side
    direction: Side
  },
  Middle{
    /// Matches the left outer region
    left: Box<VecMatcher>,
    /// Matches the left separator
    left_sep: Vec<ScalMatcher>,
    /// Matches the middle - can only ever be a plain placeholder
    mid: Box<VecMatcher>,
    /// Matches the right separator
    right_sep: Vec<ScalMatcher>,
    /// Matches the right outer region
    right: Box<VecMatcher>,
    /// Order of significance for sorting equally good solutions based on
    /// the length of matches on either side.
    /// 
    /// Vectorial keys that appear on either side, in priority order
    key_order: Vec<Token<String>>
  }
}

pub enum AnyMatcher {
  Scalar(Vec<ScalMatcher>),
  Vec{
    left: Vec<ScalMatcher>,
    mid: VecMatcher,
    right: Vec<ScalMatcher>
  }
}
impl Matcher for AnyMatcher {
  fn new(pattern: Rc<Vec<Expr>>) -> Self {
    mk_matcher(&pattern)
  }

  fn apply<'a>(&self, source: &'a [Expr]) -> Option<State<'a>> {
    any_match(self, source)
  }
}

// ################ InternedDisplay ################

fn disp_scalv(
  scalv: &Vec<ScalMatcher>,
  f: &mut std::fmt::Formatter<'_>,
  i: &Interner
) -> std::fmt::Result {
  let (head, tail) = unwrap_or!(scalv.split_first(); return Ok(()));
  head.fmt_i(f, i)?;
  for s in tail.iter() {
    write!(f, " ")?;
    s.fmt_i(f, i)?;
  }
  Ok(())
}

impl InternedDisplay for ScalMatcher {
  fn fmt_i(&self, f: &mut std::fmt::Formatter<'_>, i: &Interner) -> std::fmt::Result {
    match self {
      Self::P(p) => write!(f, "{:?}", p),
      Self::Placeh(n) => write!(f, "${}", i.r(*n)),
      Self::Name(n) => write!(f, "{}", print_nname(*n, i)),
      Self::S(c, body) => {
        f.write_char(*c)?;
        body.fmt_i(f, i)?;
        f.write_char(match c {'('=>')','['=>']','{'=>'}',_=>unreachable!()})
      },
      Self::Lambda(arg, body) => {
        f.write_char('\\')?;
        arg.fmt_i(f, i)?;
        f.write_char('.')?;
        body.fmt_i(f, i)
      }
    }
  }
}

impl InternedDisplay for VecMatcher {
  fn fmt_i(&self, f: &mut std::fmt::Formatter<'_>, i: &Interner) -> std::fmt::Result {
    match self {
      Self::Placeh { key, nonzero } => {
        if *nonzero {f.write_char('.')?;};
        write!(f, "..${}", i.r(*key))
      }
      Self::Scan { left, sep, right, direction } => {
        let arrow = match direction {
          Side::Left => " <== ",
          Side::Right => " ==> "
        };
        write!(f, "Scan{{")?;
        left.fmt_i(f, i)?;
        f.write_str(arrow)?;
        disp_scalv(sep, f, i)?;
        f.write_str(arrow)?;
        right.fmt_i(f, i)?;
        write!(f, "}}")
      },
      Self::Middle { left, left_sep, mid, right_sep, right, .. } => {
        write!(f, "Middle{{")?;
        left.fmt_i(f, i)?;
        f.write_str("|")?;
        disp_scalv(left_sep, f, i)?;
        f.write_str("|")?;
        mid.fmt_i(f, i)?;
        f.write_str("|")?;
        disp_scalv(right_sep, f, i)?;
        f.write_str("|")?;
        right.fmt_i(f, i)?;
        write!(f, "}}")
      }
    }
  }
}

impl InternedDisplay for AnyMatcher {
  fn fmt_i(&self, f: &mut std::fmt::Formatter<'_>, i: &Interner) -> std::fmt::Result {
    match self {
      Self::Scalar(s) => {
        write!(f, "(")?;
        disp_scalv(s, f, i)?;
        write!(f, ")")
      }
      Self::Vec { left, mid, right } => {
        write!(f, "[")?;
        disp_scalv(left, f, i)?;
        write!(f, "|")?;
        mid.fmt_i(f, i)?;
        write!(f, "|")?;
        disp_scalv(right, f, i)?;
        write!(f, "]")
      }
    }
  }
}