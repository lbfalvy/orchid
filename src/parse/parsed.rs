//! Datastructures representing the units of macro execution
//!
//! These structures are produced by the pipeline, processed by the macro
//! executor, and then converted to other usable formats.

use std::fmt;
use std::hash::Hash;
use std::rc::Rc;

use hashbrown::HashSet;
use intern_all::Tok;
use itertools::Itertools;
use ordered_float::NotNan;

use crate::foreign::atom::AtomGenerator;
#[allow(unused)] // for doc
use crate::interpreter::nort;
use crate::location::SourceRange;
use crate::name::{Sym, VName, VPath};
use crate::parse::numeric::print_nat16;

/// A [Clause] with associated metadata
#[derive(Clone, Debug)]
pub struct Expr {
  /// The actual value
  pub value: Clause,
  /// Information about the code that produced this value
  pub range: SourceRange,
}

impl Expr {
  /// Process all names with the given mapper.
  /// Return a new object if anything was processed
  #[must_use]
  pub fn map_names(&self, pred: &mut impl FnMut(Sym) -> Option<Sym>) -> Option<Self> {
    (self.value.map_names(pred)).map(|value| Self { value, range: self.range.clone() })
  }

  /// Visit all expressions in the tree. The search can be exited early by
  /// returning [Some]
  ///
  /// See also [crate::interpreter::nort::Expr::search_all]
  pub fn search_all<T>(&self, f: &mut impl FnMut(&Self) -> Option<T>) -> Option<T> {
    f(self).or_else(|| self.value.search_all(f))
  }
}

/// Visit all expression sequences including this sequence itself.
pub fn search_all_slcs<T>(this: &[Expr], f: &mut impl FnMut(&[Expr]) -> Option<T>) -> Option<T> {
  f(this).or_else(|| this.iter().find_map(|expr| expr.value.search_all_slcs(f)))
}

impl Expr {
  /// Add the specified prefix to every Name
  #[must_use]
  pub fn prefix(&self, prefix: &[Tok<String>], except: &impl Fn(Tok<String>) -> bool) -> Self {
    Self { value: self.value.prefix(prefix, except), range: self.range.clone() }
  }
}

impl fmt::Display for Expr {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.value.fmt(f) }
}

/// Various types of placeholders
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PHClass {
  /// Matches multiple tokens, lambdas or parenthesized groups
  Vec {
    /// If true, must match at least one clause
    nonzero: bool,
    /// Greediness in the allocation of tokens
    prio: usize,
  },
  /// Matches exactly one token, lambda or parenthesized group
  Scalar,
  /// Matches exactly one name
  Name,
}

/// Properties of a placeholder that matches unknown tokens in macros
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Placeholder {
  /// Identifier to pair placeholders in the pattern and template
  pub name: Tok<String>,
  /// The nature of the token set matched by this placeholder
  pub class: PHClass,
}

impl fmt::Display for Placeholder {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let name = &self.name;
    match self.class {
      PHClass::Scalar => write!(f, "${name}"),
      PHClass::Name => write!(f, "$_{name}"),
      PHClass::Vec { nonzero, prio } => {
        if nonzero { write!(f, "...") } else { write!(f, "..") }?;
        write!(f, "${name}:{prio}")
      },
    }
  }
}

/// Different types of brackets supported by Orchid
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum PType {
  /// ()
  Par,
  /// []
  Sqr,
  /// {}
  Curl,
}
impl PType {
  /// Left paren character for this paren type
  pub fn l(self) -> char {
    match self {
      PType::Curl => '{',
      PType::Par => '(',
      PType::Sqr => '[',
    }
  }

  /// Right paren character for this paren type
  pub fn r(self) -> char {
    match self {
      PType::Curl => '}',
      PType::Par => ')',
      PType::Sqr => ']',
    }
  }
}

/// An S-expression as read from a source file
#[derive(Debug, Clone)]
pub enum Clause {
  /// An opaque non-callable value, eg. a file handle
  Atom(AtomGenerator),
  /// A c-style name or an operator, eg. `+`, `i`, `foo::bar`
  Name(Sym),
  /// A parenthesized expression
  /// eg. `(print out "hello")`, `[1, 2, 3]`, `{Some(t) => t}`
  S(PType, Rc<Vec<Expr>>),
  /// A function expression, eg. `\x. x + 1`
  Lambda(Rc<Vec<Expr>>, Rc<Vec<Expr>>),
  /// A placeholder for macros, eg. `$name`, `...$body`, `...$lhs:1`
  Placeh(Placeholder),
}

impl Clause {
  /// Extract the expressions from an auto, lambda or S
  #[must_use]
  pub fn body(&self) -> Option<Rc<Vec<Expr>>> {
    match self {
      Self::Lambda(_, body) | Self::S(_, body) => Some(body.clone()),
      _ => None,
    }
  }

  /// Convert with identical meaning
  #[must_use]
  pub fn into_expr(self, range: SourceRange) -> Expr {
    if let Self::S(PType::Par, body) = &self {
      if let [wrapped] = &body[..] {
        return wrapped.clone();
      }
    }
    Expr { value: self, range }
  }

  /// Convert with identical meaning
  #[must_use]
  pub fn from_exprs(exprs: &[Expr]) -> Option<Self> {
    match exprs {
      [] => None,
      [only] => Some(only.value.clone()),
      _ => Some(Self::S(PType::Par, Rc::new(exprs.to_vec()))),
    }
  }

  /// Convert with identical meaning
  #[must_use]
  pub fn from_exprv(exprv: &Rc<Vec<Expr>>) -> Option<Clause> {
    if exprv.len() < 2 { Self::from_exprs(exprv) } else { Some(Self::S(PType::Par, exprv.clone())) }
  }

  /// Collect all names that appear in this expression.
  /// NOTICE: this isn't the total set of unbound names, it's mostly useful to
  /// make weak statements for optimization.
  #[must_use]
  pub fn collect_names(&self) -> HashSet<Sym> {
    if let Self::Name(n) = self {
      return HashSet::from([n.clone()]);
    }
    let mut glossary = HashSet::new();
    let result = self.search_all(&mut |e| {
      if let Clause::Name(n) = &e.value {
        glossary.insert(n.clone());
      }
      None::<()>
    });
    assert!(result.is_none(), "Callback never returns Some");
    glossary
  }

  /// Process all names with the given mapper.
  /// Return a new object if anything was processed
  #[must_use]
  pub fn map_names(&self, pred: &mut impl FnMut(Sym) -> Option<Sym>) -> Option<Self> {
    match self {
      Clause::Atom(_) | Clause::Placeh(_) => None,
      Clause::Name(name) => pred(name.clone()).map(Clause::Name),
      Clause::S(c, body) => {
        let mut any_some = false;
        let new_body = body
          .iter()
          .map(|e| {
            let val = e.map_names(pred);
            any_some |= val.is_some();
            val.unwrap_or_else(|| e.clone())
          })
          .collect();
        if any_some { Some(Clause::S(*c, Rc::new(new_body))) } else { None }
      },
      Clause::Lambda(arg, body) => {
        let mut any_some = false;
        let new_arg = (arg.iter())
          .map(|e| {
            let val = e.map_names(pred);
            any_some |= val.is_some();
            val.unwrap_or_else(|| e.clone())
          })
          .collect();
        let new_body = (body.iter())
          .map(|e| {
            let val = e.map_names(pred);
            any_some |= val.is_some();
            val.unwrap_or_else(|| e.clone())
          })
          .collect();
        if any_some { Some(Clause::Lambda(Rc::new(new_arg), Rc::new(new_body))) } else { None }
      },
    }
  }

  /// Pair of [Expr::search_all]
  pub fn search_all<T>(&self, f: &mut impl FnMut(&Expr) -> Option<T>) -> Option<T> {
    match self {
      Clause::Lambda(arg, body) =>
        arg.iter().chain(body.iter()).find_map(|expr| expr.search_all(f)),
      Clause::Name(_) | Clause::Atom(_) | Clause::Placeh(_) => None,
      Clause::S(_, body) => body.iter().find_map(|expr| expr.search_all(f)),
    }
  }

  /// Visit all expression sequences. Most useful when looking for some pattern
  pub fn search_all_slcs<T>(&self, f: &mut impl FnMut(&[Expr]) -> Option<T>) -> Option<T> {
    match self {
      Clause::Lambda(arg, body) => search_all_slcs(arg, f).or_else(|| search_all_slcs(body, f)),
      Clause::Name(_) | Clause::Atom(_) | Clause::Placeh(_) => None,
      Clause::S(_, body) => search_all_slcs(body, f),
    }
  }

  /// Generate a parenthesized expression sequence
  pub fn s(delimiter: char, body: impl IntoIterator<Item = Self>, range: SourceRange) -> Self {
    let ptype = match delimiter {
      '(' => PType::Par,
      '[' => PType::Sqr,
      '{' => PType::Curl,
      _ => panic!("not an opening paren"),
    };
    let body = body.into_iter().map(|it| it.into_expr(range.clone())).collect();
    Self::S(ptype, Rc::new(body))
  }
}

impl Clause {
  /// Add the specified prefix to every Name
  #[must_use]
  pub fn prefix(&self, prefix: &[Tok<String>], except: &impl Fn(Tok<String>) -> bool) -> Self {
    self
      .map_names(&mut |name| match except(name[0].clone()) {
        true => None,
        false => {
          let prefixed = prefix.iter().cloned().chain(name.iter()).collect::<Vec<_>>();
          Some(Sym::from_tok(name.tok().interner().i(&prefixed)).unwrap())
        },
      })
      .unwrap_or_else(|| self.clone())
  }
}

impl fmt::Display for Clause {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Atom(a) => write!(f, "{a:?}"),
      Self::Name(name) => write!(f, "{}", name),
      Self::S(t, items) => {
        let body = items.iter().join(" ");
        write!(f, "{}{body}{}", t.l(), t.r())
      },
      Self::Lambda(arg, body) => {
        let args = arg.iter().join(" ");
        let bodys = body.iter().join(" ");
        write!(f, "\\{args}.{bodys}")
      },
      Self::Placeh(ph) => ph.fmt(f),
    }
  }
}

/// A substitution rule as loaded from source
#[derive(Debug, Clone)]
pub struct Rule {
  /// Expressions on the left side of the arrow
  pub pattern: Vec<Expr>,
  /// Priority number written inside the arrow
  pub prio: NotNan<f64>,
  /// Expressions on the right side of the arrow
  pub template: Vec<Expr>,
}

impl fmt::Display for Rule {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
      f,
      "rule {} ={}=> {}",
      self.pattern.iter().join(" "),
      print_nat16(self.prio),
      self.template.iter().join(" ")
    )
  }
}

/// A named constant
#[derive(Debug, Clone)]
pub struct Constant {
  /// Used to reference the constant
  pub name: Tok<String>,
  /// The constant value inserted where the name is found
  pub value: Expr,
}

impl fmt::Display for Constant {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "const {} := {}", *self.name, self.value)
  }
}

/// An import pointing at another module, either specifying the symbol to be
/// imported or importing all available symbols with a globstar (*)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Import {
  /// Import path, a sequence of module names. Can either start with
  ///
  /// - `self` to reference the current module
  /// - any number of `super` to reference the parent module of the implied
  ///   `self`
  /// - a root name
  pub path: VPath,
  /// If name is None, this is a wildcard import
  pub name: Option<Tok<String>>,
  /// Location of the final name segment, which uniquely identifies this name
  pub range: SourceRange,
}
impl Import {
  /// Constructor
  pub fn new(
    path: impl IntoIterator<Item = Tok<String>>,
    name: Option<Tok<String>>,
    range: SourceRange,
  ) -> Self {
    let path = VPath(path.into_iter().collect());
    assert!(name.is_some() || !path.0.is_empty(), "import * not allowed");
    Self { range, name, path }
  }

  /// Get the preload target space for this import - the prefix below
  /// which all files should be included in the compilation
  ///
  /// Returns the path if this is a glob import, or the path plus the
  /// name if this is a specific import
  #[must_use]
  pub fn nonglob_path(&self) -> VName {
    VName::new(self.path.0.iter().chain(&self.name).cloned())
      .expect("Everything import (`import *`) not allowed")
  }
}

impl fmt::Display for Import {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match &self.name {
      None => write!(f, "{}::*", self.path),
      Some(n) => write!(f, "{}::{}", self.path, n),
    }
  }
}

/// A namespace block
#[derive(Debug, Clone)]
pub struct ModuleBlock {
  /// Name prefixed to all names in the block
  pub name: Tok<String>,
  /// Prefixed entries
  pub body: Vec<SourceLine>,
}

impl fmt::Display for ModuleBlock {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let bodys = self.body.iter().map(|e| e.to_string()).join("\n");
    write!(f, "module {} {{\n{}\n}}", self.name, bodys)
  }
}

/// see [Member]
#[derive(Debug, Clone)]
pub enum MemberKind {
  /// A substitution rule. Rules apply even when they're not in scope, if the
  /// absolute names are present eg. because they're produced by other rules
  Rule(Rule),
  /// A constant (or function) associated with a name
  Constant(Constant),
  /// A prefixed set of other entries
  Module(ModuleBlock),
}
impl MemberKind {
  /// Convert to [SourceLine]
  pub fn into_line(self, exported: bool, range: SourceRange) -> SourceLine {
    SourceLineKind::Member(Member { exported, kind: self }).wrap(range)
  }
}

impl fmt::Display for MemberKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Constant(c) => c.fmt(f),
      Self::Module(m) => m.fmt(f),
      Self::Rule(r) => r.fmt(f),
    }
  }
}

/// Things that may be prefixed with an export
/// see [MemberKind]
#[derive(Debug, Clone)]
pub struct Member {
  /// Various members
  pub kind: MemberKind,
  /// Whether this member is exported or not
  pub exported: bool,
}

impl fmt::Display for Member {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self { exported: true, kind } => write!(f, "export {kind}"),
      Self { exported: false, kind } => write!(f, "{kind}"),
    }
  }
}

/// See [SourceLine]
#[derive(Debug, Clone)]
pub enum SourceLineKind {
  /// Imports one or all names in a module
  Import(Vec<Import>),
  /// Comments are kept here in case dev tooling wants to parse documentation
  Comment(String),
  /// An element with visibility information
  Member(Member),
  /// A list of tokens exported explicitly. This can also create new exported
  /// tokens that the local module doesn't actually define a role for
  Export(Vec<(Tok<String>, SourceRange)>),
}
impl SourceLineKind {
  /// Wrap with no location
  pub fn wrap(self, range: SourceRange) -> SourceLine { SourceLine { kind: self, range } }
}

impl fmt::Display for SourceLineKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Comment(s) => write!(f, "--[{s}]--"),
      Self::Export(s) => {
        write!(f, "export ::({})", s.iter().map(|t| &**t.0).join(", "))
      },
      Self::Member(member) => write!(f, "{member}"),
      Self::Import(i) => {
        write!(f, "import ({})", i.iter().map(|i| i.to_string()).join(", "))
      },
    }
  }
}

/// Anything the parser might encounter in a file. See [SourceLineKind]
#[derive(Debug, Clone)]
pub struct SourceLine {
  /// What we encountered
  pub kind: SourceLineKind,
  /// Where we encountered it.
  pub range: SourceRange,
}

impl fmt::Display for SourceLine {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { self.kind.fmt(f) }
}
