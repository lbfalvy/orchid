use std::fmt::{Display, Write};
use std::rc::Rc;

use itertools::Itertools;

use super::any_match::any_match;
use super::build::mk_any;
use crate::foreign::Atom;
use crate::interner::Tok;
use crate::rule::matcher::{Matcher, RuleExpr};
use crate::rule::state::State;
use crate::utils::Side;
use crate::{Sym, VName};

pub enum ScalMatcher {
  Atom(Atom),
  Name(Sym),
  S(char, Box<AnyMatcher>),
  Lambda(Box<AnyMatcher>, Box<AnyMatcher>),
  Placeh(Tok<String>),
}

pub enum VecMatcher {
  Placeh {
    key: Tok<String>,
    nonzero: bool,
  },
  Scan {
    left: Box<VecMatcher>,
    sep: Vec<ScalMatcher>,
    right: Box<VecMatcher>,
    /// The separator traverses the sequence towards this side
    direction: Side,
  },
  Middle {
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
    key_order: VName,
  },
}

pub enum AnyMatcher {
  Scalar(Vec<ScalMatcher>),
  Vec { left: Vec<ScalMatcher>, mid: VecMatcher, right: Vec<ScalMatcher> },
}
impl Matcher for AnyMatcher {
  fn new(pattern: Rc<Vec<RuleExpr>>) -> Self { mk_any(&pattern) }

  fn apply<'a>(&self, source: &'a [RuleExpr]) -> Option<State<'a>> {
    any_match(self, source)
  }
}

// ################ Display ################

impl Display for ScalMatcher {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Atom(a) => write!(f, "{a:?}"),
      Self::Placeh(n) => write!(f, "${n}"),
      Self::Name(n) => write!(f, "{}", n.extern_vec().join("::")),
      Self::S(c, body) => {
        let pair = match c {
          '(' => ')',
          '[' => ']',
          '{' => '}',
          _ => unreachable!(),
        };
        write!(f, "{c}{body}{pair}")
      },
      Self::Lambda(arg, body) => {
        write!(f, "\\{arg}.{body}")
      },
    }
  }
}

impl Display for VecMatcher {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Placeh { key, nonzero } => {
        if *nonzero {
          f.write_char('.')?;
        };
        write!(f, "..${key}")
      },
      Self::Scan { left, sep, right, direction } => match direction {
        Side::Left => {
          write!(f, "Scan{{{left} <== {} <== {right}}}", sep.iter().join(" "))
        },
        Side::Right => {
          write!(f, "Scan{{{left} ==> {} ==> {right}}}", sep.iter().join(" "))
        },
      },
      Self::Middle { left, left_sep, mid, right_sep, right, .. } => {
        let left_sep_s = left_sep.iter().join(" ");
        let right_sep_s = right_sep.iter().join(" ");
        write!(f, "Middle{{{left}|{left_sep_s}|{mid}|{right_sep_s}|{right}}}")
      },
    }
  }
}

impl Display for AnyMatcher {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::Scalar(s) => {
        write!(f, "({})", s.iter().join(" "))
      },
      Self::Vec { left, mid, right } => {
        let lefts = left.iter().join(" ");
        let rights = right.iter().join(" ");
        write!(f, "[{lefts}|{mid}|{rights}]")
      },
    }
  }
}

// ################ External ################

/// A [Matcher] implementation that builds a priority-order tree of the
/// vectorial placeholders and handles the scalars on leaves.
pub struct VectreeMatcher(AnyMatcher);
impl Matcher for VectreeMatcher {
  fn new(pattern: Rc<Vec<RuleExpr>>) -> Self { Self(AnyMatcher::new(pattern)) }

  fn apply<'a>(&self, source: &'a [RuleExpr]) -> Option<State<'a>> {
    self.0.apply(source)
  }
}
impl Display for VectreeMatcher {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.0.fmt(f)
  }
}
