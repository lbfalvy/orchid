//! Datastructures for cached pattern

use std::fmt;
use std::rc::Rc;

use intern_all::Tok;
use itertools::Itertools;

use super::any_match::any_match;
use super::build::mk_any;
use crate::foreign::atom::AtomGenerator;
use crate::name::Sym;
use crate::parse::parsed::PType;
use crate::rule::matcher::{Matcher, RuleExpr};
use crate::rule::state::State;
use crate::utils::side::Side;

pub(super) enum ScalMatcher {
  Atom(AtomGenerator),
  Name(Sym),
  S(PType, Box<AnyMatcher>),
  Lambda(Box<AnyMatcher>, Box<AnyMatcher>),
  Placeh { key: Tok<String>, name_only: bool },
}

pub(super) enum VecMatcher {
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
    /// Order of significance for sorting equally good projects based on
    /// the length of matches on either side.
    ///
    /// Vectorial keys that appear on either side, in priority order
    key_order: Vec<Tok<String>>,
  },
}

pub(super) enum AnyMatcher {
  Scalar(Vec<ScalMatcher>),
  Vec { left: Vec<ScalMatcher>, mid: VecMatcher, right: Vec<ScalMatcher> },
}
impl Matcher for AnyMatcher {
  fn new(pattern: Rc<Vec<RuleExpr>>) -> Self { mk_any(&pattern) }

  fn apply<'a>(
    &self,
    source: &'a [RuleExpr],
    save_loc: &impl Fn(Sym) -> bool,
  ) -> Option<State<'a>> {
    any_match(self, source, save_loc)
  }
}

// ################ Display ################

impl fmt::Display for ScalMatcher {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Atom(a) => write!(f, "{a:?}"),
      Self::Placeh { key, name_only } => match name_only {
        false => write!(f, "${key}"),
        true => write!(f, "$_{key}"),
      },
      Self::Name(n) => write!(f, "{n}"),
      Self::S(t, body) => write!(f, "{}{body}{}", t.l(), t.r()),
      Self::Lambda(arg, body) => write!(f, "\\{arg}.{body}"),
    }
  }
}

impl fmt::Display for VecMatcher {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Placeh { key, nonzero: true } => write!(f, "...${key}"),
      Self::Placeh { key, nonzero: false } => write!(f, "..${key}"),
      Self::Scan { left, sep, right, direction } => {
        let arrow = if direction == &Side::Left { "<==" } else { "==>" };
        write!(f, "Scan{{{left} {arrow} {} {arrow} {right}}}", sep.iter().join(" "))
      },
      Self::Middle { left, left_sep, mid, right_sep, right, .. } => {
        let left_sep_s = left_sep.iter().join(" ");
        let right_sep_s = right_sep.iter().join(" ");
        write!(f, "Middle{{{left}|{left_sep_s}|{mid}|{right_sep_s}|{right}}}")
      },
    }
  }
}

impl fmt::Display for AnyMatcher {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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

  fn apply<'a>(
    &self,
    source: &'a [RuleExpr],
    save_loc: &impl Fn(Sym) -> bool,
  ) -> Option<State<'a>> {
    self.0.apply(source, save_loc)
  }
}
impl fmt::Display for VectreeMatcher {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.0.fmt(f) }
}
