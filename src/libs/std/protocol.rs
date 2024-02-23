//! Polymorphism through Elixir-style protocols associated with type-tagged
//! values. A type-tag seals the value, and can only be unwrapped explicitly by
//! providing the correct type. This ensures that code that uses this explicit
//! polymorphism never uses the implicit polymorphism of dynamic typing.
//!
//! Atoms can also participate in this controlled form of polymorphism by
//! offering a [Tag] in their [crate::utils::ddispatch::Responder] callback.
//!
//! Protocols and types are modules with magic elements that distinguish them
//! from regular modules.

use std::sync::Arc;
use std::{fmt, iter};

use const_format::formatcp;
use hashbrown::HashMap;
use intern_all::{i, Tok};
use itertools::Itertools;

use super::cross_pipeline::defer_to_runtime;
use super::reflect::refer_seq;
use super::runtime_error::RuntimeError;
use crate::error::ProjectResult;
use crate::foreign::atom::Atomic;
use crate::foreign::error::RTResult;
use crate::foreign::inert::{Inert, InertPayload};
use crate::foreign::process::Unstable;
use crate::gen::tpl;
use crate::gen::traits::GenClause;
use crate::gen::tree::{atom_ent, leaf, xfn_ent, ConstTree};
use crate::interpreter::nort;
use crate::interpreter::nort::ClauseInst;
use crate::libs::parse_custom_line::custom_line;
use crate::location::SourceRange;
use crate::name::{Sym, VName};
use crate::parse::frag::Frag;
use crate::parse::lexer::Lexeme;
use crate::parse::parse_plugin::{ParseLinePlugin, ParsePluginReq};
use crate::parse::parsed::{
  self, Constant, Member, MemberKind, ModuleBlock, PType, SourceLine, SourceLineKind,
};
use crate::utils::ddispatch::Request;

// TODO: write an example that thoroughly tests this module. Test rust-interop
// with Tuple

/// Information available both for protocols and for tagged values
#[derive(Clone)]
pub struct TypeData {
  /// The full path of the module designated to represent this type or protocol.
  /// Also used to key impl tables in the counterpart (tag or protocol)
  pub id: Sym,
  /// Maps IDs of the counterpart (tag or protocol) to implementations
  pub impls: Arc<HashMap<Sym, nort::Expr>>,
}
impl TypeData {
  /// Create a new type data record from a known name and impls
  pub fn new(id: Sym, impls: impl IntoIterator<Item = (Sym, nort::Expr)>) -> Self {
    Self { id, impls: Arc::new(impls.into_iter().collect()) }
  }
}

fn mk_mod<'a>(
  rest: impl IntoIterator<Item = (&'a str, ConstTree)>,
  impls: HashMap<Sym, nort::Expr>,
  profile: ImplsProfile<impl WrapImpl>,
) -> ConstTree {
  ConstTree::tree(rest.into_iter().chain([
    (profile.own_id, leaf(tpl::A(tpl::C("std::reflect::modname"), tpl::V(Inert(1))))),
    atom_ent(TYPE_KEY, [use_wrap(profile.wrap, impls)]),
  ]))
}

fn to_mod<'a>(
  rest: impl IntoIterator<Item = (&'a str, ConstTree)>,
  data: TypeData,
  profile: ImplsProfile<impl WrapImpl>,
) -> ConstTree {
  let id = data.id.clone();
  ConstTree::tree(rest.into_iter().chain([
    atom_ent(profile.own_id, [Unstable::new(move |r| {
      assert!(r.location.module == id, "Pre-initilaized type lib mounted on wrong prefix");
      Inert(r.location.module)
    })]),
    atom_ent(TYPE_KEY, [profile.wrap.wrap(data)]),
  ]))
}

/// Key for type data. The value is either [Inert<Protocol>] or [Inert<Tag>]
const TYPE_KEY: &str = "__type_data__";

/// A shared behaviour that may implement itself for types, and may be
/// implemented by types.
#[derive(Clone)]
pub struct Protocol(pub TypeData);
impl Protocol {
  /// Name of the member the ID must be assigned to for a module to be
  /// recognized as a protocol.
  pub const ID_KEY: &'static str = "__protocol_id__";
  const fn profile() -> ImplsProfile<impl WrapImpl> {
    ImplsProfile {
      wrap: |t| Inert(Protocol(t)),
      own_id: Protocol::ID_KEY,
      other_id: Tag::ID_KEY,
      prelude: formatcp!(
        "import std
        const {} := std::reflect::modname 1
        const resolve := std::protocol::resolve {TYPE_KEY}
        const vcall := std::protocol::vcall {TYPE_KEY}",
        Protocol::ID_KEY
      ),
    }
  }

  /// Create a new protocol with a pre-determined name
  pub fn new(id: Sym, impls: impl IntoIterator<Item = (Sym, nort::Expr)>) -> Self {
    Self(TypeData::new(id, impls))
  }

  /// Attach a pre-existing protocol to the tree. Consider [Protocol::tree].
  pub fn to_tree<'a>(&self, rest: impl IntoIterator<Item = (&'a str, ConstTree)>) -> ConstTree {
    to_mod(rest, self.0.clone(), Self::profile())
  }

  /// Create a new protocol definition
  pub fn tree<'a>(
    impls: impl IntoIterator<Item = (Sym, nort::Expr)>,
    rest: impl IntoIterator<Item = (&'a str, ConstTree)>,
  ) -> ConstTree {
    mk_mod(rest, impls.into_iter().collect(), Self::profile())
  }
}
impl fmt::Debug for Protocol {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Protocol({})", self.0.id) }
}
impl InertPayload for Protocol {
  const TYPE_STR: &'static str = "Protocol";
}

/// A type marker that can be attached to values to form a [Tagged]
#[derive(Clone)]
pub struct Tag(pub TypeData);
impl Tag {
  const ID_KEY: &'static str = "__type_id__";
  const fn profile() -> ImplsProfile<impl WrapImpl> {
    ImplsProfile {
      wrap: |t| Inert(Tag(t)),
      own_id: Tag::ID_KEY,
      other_id: Protocol::ID_KEY,
      prelude: formatcp!(
        "import std
        const {} := std::reflect::modname 1
        const unwrap := std::protocol::unwrap {TYPE_KEY}
        const wrap := std::protocol::wrap {TYPE_KEY}",
        Tag::ID_KEY
      ),
    }
  }

  /// Create a new type-tag with a pre-determined name
  pub fn new(id: Sym, impls: impl IntoIterator<Item = (Sym, nort::Expr)>) -> Self {
    Self(TypeData::new(id, impls))
  }

  /// Attach a pre-existing type-tag to the tree. Consider [Tag::tree]
  pub fn to_tree<'a>(&self, rest: impl IntoIterator<Item = (&'a str, ConstTree)>) -> ConstTree {
    to_mod(rest, self.0.clone(), Self::profile())
  }

  /// Create a new tag
  pub fn tree<'a>(
    impls: impl IntoIterator<Item = (Sym, nort::Expr)>,
    rest: impl IntoIterator<Item = (&'a str, ConstTree)>,
  ) -> ConstTree {
    mk_mod(rest, impls.into_iter().collect(), Self::profile())
  }
}
impl fmt::Debug for Tag {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "Tag({})", self.0.id) }
}
impl InertPayload for Tag {
  const TYPE_STR: &'static str = "Tag";
  fn strict_eq(&self, other: &Self) -> bool { self.0.id == other.0.id }
}

/// A value with a type [Tag]
#[derive(Clone)]
pub struct Tagged {
  /// Type information
  pub tag: Tag,
  /// Value
  pub value: nort::Expr,
}
impl fmt::Debug for Tagged {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "Tagged({:?} {})", self.tag, self.value)
  }
}
impl InertPayload for Tagged {
  const TYPE_STR: &'static str = "Tagged";
  fn respond(&self, mut request: Request) { request.serve_with(|| self.tag.clone()) }
}

fn parse_impl(
  tail: Frag,
  req: &dyn ParsePluginReq,
) -> Option<ProjectResult<(VName, parsed::Expr)>> {
  custom_line(tail, i!(str: "impl"), false, req).map(|res| {
    let (_, tail, _) = res?;
    let (name, tail) = req.parse_nsname(tail)?;
    let (walrus, tail) = req.pop(tail.trim())?;
    req.expect(Lexeme::Walrus, walrus)?;
    let (body, empty) = req.parse_exprv(tail, None)?;
    req.expect_empty(empty)?;
    let value = req.vec_to_single(tail.fallback, body)?;
    Ok((name, value))
  })
}

struct Impl {
  target: Sym,
  value: parsed::Expr,
}

fn extract_impls(
  tail: Frag,
  req: &dyn ParsePluginReq,
  range: SourceRange,
  typeid_name: Tok<String>,
) -> ProjectResult<(Vec<SourceLine>, Vec<Impl>)> {
  let mut lines = Vec::new();
  let mut impls = Vec::new(); // name1, value1, name2, value2, etc...
  for line in req.split_lines(tail) {
    match parse_impl(line, req) {
      Some(result) => {
        let (name, value) = result?;
        let target = Sym::new(name.suffix([typeid_name.clone()])).unwrap();
        impls.push(Impl { target, value });
      },
      None => lines.extend((req.parse_line(line)?.into_iter()).map(|k| k.wrap(range.clone()))),
    }
  }
  Ok((lines, impls))
}

trait WrapImpl: Clone + Copy + Send + Sync + 'static {
  type R: Atomic + Clone + 'static;
  fn wrap(&self, data: TypeData) -> Self::R;
}
impl<R: Atomic + Clone + 'static, F: Fn(TypeData) -> R + Clone + Copy + Send + Sync + 'static>
  WrapImpl for F
{
  type R = R;
  fn wrap(&self, data: TypeData) -> Self::R { self(data) }
}
fn use_wrap(wrap: impl WrapImpl, impls: HashMap<Sym, nort::Expr>) -> impl Atomic + Clone + 'static {
  Unstable::new(move |r| wrap.wrap(TypeData::new(r.location.module, impls)))
}

#[derive(Debug, Clone)]
struct ImplsProfile<W: WrapImpl> {
  wrap: W,
  own_id: &'static str,
  other_id: &'static str,
  prelude: &'static str,
}

fn parse_body_with_impls(
  body: Frag,
  req: &dyn ParsePluginReq,
  range: SourceRange,
  profile: ImplsProfile<impl WrapImpl>,
) -> ProjectResult<Vec<SourceLine>> {
  let ImplsProfile { other_id, prelude, wrap, .. } = profile.clone();
  let (mut lines, impls) = extract_impls(body, req, range.clone(), i(other_id))?;
  let line_loc = range.clone();
  let type_data = defer_to_runtime(
    range.clone(),
    impls.into_iter().flat_map(move |Impl { target, value }| {
      [vec![parsed::Clause::Name(target).into_expr(line_loc.clone())], vec![value]]
    }),
    move |pairs: Vec<nort::Expr>| -> RTResult<_> {
      debug_assert_eq!(pairs.len() % 2, 0, "key-value pairs");
      let mut impls = HashMap::with_capacity(pairs.len() / 2);
      for (name, value) in pairs.into_iter().tuples() {
        impls.insert(name.downcast::<Inert<Sym>>()?.0, value);
      }
      Ok(use_wrap(wrap, impls))
    },
  );
  let type_data_line = Constant { name: i(TYPE_KEY), value: type_data.into_expr(range.clone()) };
  lines.extend(req.parse_entries(prelude, range.clone()));
  lines.push(MemberKind::Constant(type_data_line).into_line(true, range));
  Ok(lines)
}

#[derive(Clone)]
struct ProtocolParser;
impl ParseLinePlugin for ProtocolParser {
  fn parse(&self, req: &dyn ParsePluginReq) -> Option<ProjectResult<Vec<SourceLineKind>>> {
    custom_line(req.frag(), i!(str: "protocol"), true, req).map(|res| {
      let (exported, tail, line_loc) = res?;
      let (name, tail) = req.pop(tail)?;
      let name = req.expect_name(name)?;
      let tail = req.expect_block(tail, PType::Par)?;
      let body = parse_body_with_impls(tail, req, line_loc, Protocol::profile())?;
      let kind = MemberKind::Module(ModuleBlock { name, body });
      Ok(vec![SourceLineKind::Member(Member { exported, kind })])
    })
  }
}

#[derive(Clone)]
struct TypeParser;
impl ParseLinePlugin for TypeParser {
  fn parse(&self, req: &dyn ParsePluginReq) -> Option<ProjectResult<Vec<SourceLineKind>>> {
    custom_line(req.frag(), i!(str: "type"), true, req).map(|res| {
      let (exported, tail, line_loc) = res?;
      let (name, tail) = req.pop(tail)?;
      let name = req.expect_name(name)?;
      let tail = req.expect_block(tail, PType::Par)?;
      let body = parse_body_with_impls(tail, req, line_loc, Tag::profile())?;
      let kind = MemberKind::Module(ModuleBlock { name, body });
      Ok(vec![SourceLineKind::Member(Member { exported, kind })])
    })
  }
}

#[derive(Clone)]
struct AsProtocolParser;
impl ParseLinePlugin for AsProtocolParser {
  fn parse(&self, req: &dyn ParsePluginReq) -> Option<ProjectResult<Vec<SourceLineKind>>> {
    custom_line(req.frag(), i!(str: "as_protocol"), false, req).map(|res| {
      let (_, tail, line_loc) = res?;
      let body = req.expect_block(tail, PType::Par)?;
      parse_body_with_impls(body, req, line_loc, Protocol::profile())
        .map(|v| v.into_iter().map(|e| e.kind).collect())
    })
  }
}

#[derive(Clone)]
struct AsTypeParser;
impl ParseLinePlugin for AsTypeParser {
  fn parse(&self, req: &dyn ParsePluginReq) -> Option<ProjectResult<Vec<SourceLineKind>>> {
    custom_line(req.frag(), i!(str: "as_type"), false, req).map(|res| {
      let (_, tail, line_loc) = res?;
      let body = req.expect_block(tail, PType::Par)?;
      parse_body_with_impls(body, req, line_loc, Tag::profile())
        .map(|v| v.into_iter().map(|e| e.kind).collect())
    })
  }
}

/// Collection of all the parser plugins defined here
pub fn parsers() -> Vec<Box<dyn ParseLinePlugin>> {
  vec![
    Box::new(ProtocolParser),
    Box::new(TypeParser),
    Box::new(AsTypeParser),
    Box::new(AsProtocolParser),
  ]
}

/// Check and remove the type tag from a value
pub fn unwrap(tag: Inert<Tag>, tagged: Inert<Tagged>) -> RTResult<nort::Expr> {
  if tagged.tag.strict_eq(&tag) {
    return Ok(tagged.value.clone());
  }
  let msg = format!("expected {:?} but got {:?}", tag, tagged.tag);
  RuntimeError::fail(msg, "unwrapping type-tagged value")
}

/// Attach a type tag to a value
pub fn wrap(tag: Inert<Tag>, value: nort::Expr) -> Inert<Tagged> {
  Inert(Tagged { tag: tag.0, value })
}

/// Find the implementation of a protocol for a given value
pub fn resolve(protocol: Inert<Protocol>, value: ClauseInst) -> RTResult<nort::Expr> {
  let tag = value.request::<Tag>().ok_or_else(|| {
    let msg = format!("{value} is not type-tagged");
    RuntimeError::ext(msg, "resolving protocol impl")
  })?;
  if let Some(implem) = protocol.0.0.impls.get(&tag.0.id) {
    Ok(implem.clone())
  } else if let Some(implem) = tag.0.impls.get(&protocol.0.0.id) {
    Ok(implem.clone())
  } else {
    let message = format!("{tag:?} doesn't implement {protocol:?}");
    RuntimeError::fail(message, "dispatching protocol")
  }
}

/// Generate a call to [resolve] bound to the given protocol
pub const fn gen_resolv(name: &'static str) -> impl GenClause {
  tpl::A(
    tpl::C("std::protocol::resolve"),
    tpl::V(Unstable::new(move |_| refer_seq(name.split("::").chain(iter::once(TYPE_KEY))))),
  )
}

/// All the functions exposed by the std::protocol library
pub fn protocol_lib() -> ConstTree {
  ConstTree::ns("std::protocol", [ConstTree::tree([
    xfn_ent("unwrap", [unwrap]),
    xfn_ent("wrap", [wrap]),
    xfn_ent("resolve", [resolve]),
    xfn_ent("break", [|t: Inert<Tagged>| t.0.value]),
  ])])
}
