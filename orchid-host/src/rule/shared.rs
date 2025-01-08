//! Datastructures for cached pattern

use std::fmt;

use itertools::Itertools;
use orchid_base::interner::Tok;
use orchid_base::name::Sym;
use orchid_base::side::Side;
use orchid_base::tokens::{PARENS, Paren};

pub enum ScalMatcher {
  Name(Sym),
  S(Paren, Box<AnyMatcher>),
  Lambda(Box<AnyMatcher>, Box<AnyMatcher>),
  Placeh { key: Tok<String> },
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
    /// Order of significance for sorting equally good projects based on
    /// the length of matches on either side.
    ///
    /// Vectorial keys that appear on either side, in priority order
    key_order: Vec<Tok<String>>,
  },
}

pub enum AnyMatcher {
  Scalar(Vec<ScalMatcher>),
  Vec { left: Vec<ScalMatcher>, mid: VecMatcher, right: Vec<ScalMatcher> },
}

// ################ Display ################

impl fmt::Display for ScalMatcher {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Placeh { key } => write!(f, "${key}"),
      Self::Name(n) => write!(f, "{n}"),
      Self::S(t, body) => {
        let (l, r, _) = PARENS.iter().find(|r| r.2 == *t).unwrap();
        write!(f, "{l}{body}{r}")
      },
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
