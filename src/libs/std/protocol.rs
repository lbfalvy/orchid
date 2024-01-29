use std::fmt::Debug;
use std::iter;
use std::sync::Arc;

use const_format::formatcp;
use hashbrown::HashMap;
use intern_all::{i, Tok};
use itertools::Itertools;

use super::cross_pipeline::defer_to_runtime;
use super::reflect::{refer_seq, RefEqual};
use super::runtime_error::RuntimeError;
use crate::error::ProjectResult;
use crate::foreign::atom::Atomic;
use crate::foreign::error::ExternResult;
use crate::foreign::inert::{Inert, InertPayload};
use crate::foreign::process::Unstable;
use crate::foreign::to_clause::ToClause;
use crate::gen::tpl;
use crate::gen::traits::GenClause;
use crate::gen::tree::{atom_leaf, xfn_ent, ConstTree};
use crate::interpreter::nort as int;
use crate::interpreter::nort::ClauseInst;
use crate::libs::parse_custom_line::custom_line;
use crate::location::SourceRange;
use crate::name::{Sym, VName};
use crate::parse::frag::Frag;
use crate::parse::lexer::Lexeme;
use crate::parse::parse_plugin::{ParseLinePlugin, ParsePluginReq};
use crate::parse::parsed::{
  self, Constant, Member, MemberKind, ModuleBlock, PType, SourceLine,
  SourceLineKind,
};
use crate::utils::ddispatch::Request;

pub struct TypeData {
  pub id: RefEqual,
  pub display_name: Tok<String>,
  pub impls: HashMap<RefEqual, int::Expr>,
}

/// Key for type data. The value is either [Inert<Protocol>] or [Inert<Tag>]
const TYPE_KEY: &str = "__type_data__";

#[derive(Clone)]
pub struct Protocol(pub Arc<TypeData>);
impl Protocol {
  const ID_KEY: &'static str = "__protocol_id__";

  pub fn new_id(
    id: RefEqual,
    display_name: Tok<String>,
    impls: impl IntoIterator<Item = (RefEqual, int::Expr)>,
  ) -> Self {
    let impls = impls.into_iter().collect();
    Protocol(Arc::new(TypeData { id, display_name, impls }))
  }

  pub fn new(
    display_name: &'static str,
    impls: impl IntoIterator<Item = (RefEqual, int::Expr)>,
  ) -> Self {
    Self::new_id(RefEqual::new(), i(display_name), impls)
  }

  pub fn id(&self) -> RefEqual { self.0.id.clone() }

  pub fn as_tree_ent<'a>(
    &'a self,
    rest: impl IntoIterator<Item = (&'a str, ConstTree)>,
  ) -> (&str, ConstTree) {
    ConstTree::tree_ent(
      self.0.display_name.as_str(),
      rest.into_iter().chain([
        (Self::ID_KEY, atom_leaf(Inert(self.id()))),
        (TYPE_KEY, atom_leaf(Inert(self.clone()))),
      ]),
    )
  }
}
impl Debug for Protocol {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple(&self.0.display_name).field(&self.0.id.id()).finish()
  }
}
impl InertPayload for Protocol {
  const TYPE_STR: &'static str = "Protocol";
}

#[derive(Clone)]
pub struct Tag(pub Arc<TypeData>);
impl Tag {
  const ID_KEY: &'static str = "__type_id__";

  pub fn new_id(
    id: RefEqual,
    display_name: Tok<String>,
    impls: impl IntoIterator<Item = (RefEqual, int::Expr)>,
  ) -> Self {
    let impls = impls.into_iter().collect();
    Self(Arc::new(TypeData { id, display_name, impls }))
  }

  pub fn new(
    display_name: &'static str,
    impls: impl IntoIterator<Item = (RefEqual, int::Expr)>,
  ) -> Self {
    Self::new_id(RefEqual::new(), i(display_name), impls)
  }

  pub fn id(&self) -> RefEqual { self.0.id.clone() }

  pub fn as_tree_ent<'a>(
    &'a self,
    rest: impl IntoIterator<Item = (&'a str, ConstTree)>,
  ) -> (&str, ConstTree) {
    ConstTree::tree_ent(
      self.0.display_name.as_str(),
      rest.into_iter().chain([
        (Self::ID_KEY, atom_leaf(Inert(self.id()))),
        (TYPE_KEY, atom_leaf(Inert(self.clone()))),
      ]),
    )
  }
}
impl Debug for Tag {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple(&self.0.display_name).field(&self.0.id.id()).finish()
  }
}
impl InertPayload for Tag {
  const TYPE_STR: &'static str = "Tag";
  fn strict_eq(&self, other: &Self) -> bool { self.0.id == other.0.id }
}

#[derive(Clone)]
pub struct Tagged {
  pub tag: Tag,
  pub value: int::Expr,
}
impl Debug for Tagged {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("Tagged").field(&self.tag).field(&self.value).finish()
  }
}
impl InertPayload for Tagged {
  const TYPE_STR: &'static str = "Tagged";
  fn respond(&self, mut request: Request) {
    request.serve_with(|| self.tag.clone())
  }
}

fn parse_impl(
  tail: Frag,
  req: &dyn ParsePluginReq,
) -> Option<ProjectResult<(VName, parsed::Expr)>> {
  custom_line(tail, i("impl"), false, req).map(|res| {
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
        let target =
          Sym::new(name.suffix([typeid_name.clone()])).unwrap();
        impls.push(Impl { target, value });
      },
      None => lines.extend(
        (req.parse_line(line)?.into_iter()).map(|k| k.wrap(range.clone())),
      ),
    }
  }
  Ok((lines, impls))
}

trait WrapImpl: Clone + Send + Sync + 'static {
  type R: ToClause;
  fn wrap(&self, data: Arc<TypeData>) -> Self::R;
}
impl<R: ToClause, F: Fn(Arc<TypeData>) -> R + Clone + Send + Sync + 'static>
  WrapImpl for F
{
  type R = R;
  fn wrap(&self, data: Arc<TypeData>) -> Self::R { self(data) }
}

struct ImplsProfile<'a, W: WrapImpl> {
  wrap: W,
  own_id: Tok<String>,
  other_id: Tok<String>,
  prelude: &'a str,
}

fn parse_body_with_impls<W: WrapImpl>(
  display_name: Tok<String>,
  body: Frag,
  req: &dyn ParsePluginReq,
  range: SourceRange,
  profile: &ImplsProfile<'static, W>,
) -> ProjectResult<Vec<SourceLine>> {
  let id = RefEqual::new();
  let (lines, impls) =
    extract_impls(body, req, range.clone(), profile.other_id.clone())?;

  Ok(
    req
      .parse_entries(profile.prelude, range.clone())
      .into_iter()
      .chain(
        [
          (profile.own_id.clone(), Inert(id.clone()).ast_cls()),
          (
            i(TYPE_KEY),
            defer_to_runtime(
              range.clone(),
              impls.into_iter().flat_map({
                let line_loc = range.clone();
                move |Impl { target, value }| {
                  [
                    parsed::Clause::Name(target).into_expr(line_loc.clone()),
                    value,
                  ]
                  .map(|e| ((), vec![e]))
                }
              }),
              {
                let display_name = display_name.clone();
                let wrap = profile.wrap.clone();
                move |pairs: Vec<((), int::Expr)>| -> ExternResult<_> {
                  let mut impls = HashMap::new();
                  debug_assert_eq!(pairs.len() % 2, 0, "key-value pairs");
                  let mut nvnvnv = pairs.into_iter().map(|t| t.1);
                  while let Some((name, value)) = nvnvnv.next_tuple() {
                    let key = name.downcast::<Inert<RefEqual>>()?;
                    impls.insert(key.0, value);
                  }
                  let id = id.clone();
                  let display_name = display_name.clone();
                  Ok(wrap.wrap(Arc::new(TypeData { id, display_name, impls })))
                }
              },
            ),
          ),
        ]
        .map(|(name, value)| {
          let value = parsed::Expr { value, range: range.clone() };
          MemberKind::Constant(Constant { name, value })
            .to_line(true, range.clone())
        }),
      )
      .chain(lines)
      .collect(),
  )
}

fn protocol_impls_profile() -> ImplsProfile<'static, impl WrapImpl> {
  ImplsProfile {
    wrap: |t| Inert(Protocol(t)),
    own_id: i(Protocol::ID_KEY),
    other_id: i(Tag::ID_KEY),
    prelude: formatcp!(
      "import std::protocol
      const resolve := protocol::resolve {TYPE_KEY}
      const get_impl := protocol::get_impl {TYPE_KEY}"
    ),
  }
}

fn type_impls_profile() -> ImplsProfile<'static, impl WrapImpl> {
  ImplsProfile {
    wrap: |t| Inert(Tag(t)),
    own_id: i(Tag::ID_KEY),
    other_id: i(Protocol::ID_KEY),
    prelude: formatcp!(
      "import std::protocol
      const unwrap := protocol::unwrap {TYPE_KEY}
      const wrap := protocol::wrap {TYPE_KEY}"
    ),
  }
}

struct ProtocolParser;
impl ParseLinePlugin for ProtocolParser {
  fn parse(
    &self,
    req: &dyn ParsePluginReq,
  ) -> Option<ProjectResult<Vec<SourceLineKind>>> {
    custom_line(req.frag(), i("protocol"), true, req).map(|res| {
      let (exported, tail, line_loc) = res?;
      let (name, tail) = req.pop(tail)?;
      let name = req.expect_name(name)?;
      let tail = req.expect_block(tail, PType::Par)?;
      let profile = protocol_impls_profile();
      let body =
        parse_body_with_impls(name.clone(), tail, req, line_loc, &profile)?;
      let kind = MemberKind::Module(ModuleBlock { name, body });
      Ok(vec![SourceLineKind::Member(Member { exported, kind })])
    })
  }
}

struct TypeParser;
impl ParseLinePlugin for TypeParser {
  fn parse(
    &self,
    req: &dyn ParsePluginReq,
  ) -> Option<ProjectResult<Vec<SourceLineKind>>> {
    custom_line(req.frag(), i("type"), true, req).map(|res| {
      let (exported, tail, line_loc) = res?;
      let (name, tail) = req.pop(tail)?;
      let name = req.expect_name(name)?;
      let tail = req.expect_block(tail, PType::Par)?;
      let profile = type_impls_profile();
      let body =
        parse_body_with_impls(name.clone(), tail, req, line_loc, &profile)?;
      let kind = MemberKind::Module(ModuleBlock { name, body });
      Ok(vec![SourceLineKind::Member(Member { exported, kind })])
    })
  }
}

struct AsProtocolParser;
impl ParseLinePlugin for AsProtocolParser {
  fn parse(
    &self,
    req: &dyn ParsePluginReq,
  ) -> Option<ProjectResult<Vec<SourceLineKind>>> {
    custom_line(req.frag(), i("as_protocol"), false, req).map(|res| {
      let (_, tail, line_loc) = res?;
      let (name, tail) = req.pop(tail)?;
      let name = req.expect_name(name)?;
      let body = req.expect_block(tail, PType::Par)?;
      let profile = protocol_impls_profile();
      parse_body_with_impls(name, body, req, line_loc, &profile)
        .map(|v| v.into_iter().map(|e| e.kind).collect())
    })
  }
}

struct AsTypeParser;
impl ParseLinePlugin for AsTypeParser {
  fn parse(
    &self,
    req: &dyn ParsePluginReq,
  ) -> Option<ProjectResult<Vec<SourceLineKind>>> {
    custom_line(req.frag(), i("as_type"), false, req).map(|res| {
      let (_, tail, line_loc) = res?;
      let (name, tail) = req.pop(tail)?;
      let name = req.expect_name(name)?;
      let body = req.expect_block(tail, PType::Par)?;
      let profile = type_impls_profile();
      parse_body_with_impls(name, body, req, line_loc, &profile)
        .map(|v| v.into_iter().map(|e| e.kind).collect())
    })
  }
}

pub fn parsers() -> Vec<Box<dyn ParseLinePlugin>> {
  vec![
    Box::new(ProtocolParser),
    Box::new(TypeParser),
    Box::new(AsTypeParser),
    Box::new(AsProtocolParser),
  ]
}

pub fn unwrap(
  tag: Inert<Tag>,
  tagged: Inert<Tagged>,
) -> ExternResult<int::Expr> {
  if tagged.tag.strict_eq(&tag) {
    return Ok(tagged.value.clone());
  }
  let msg = format!("expected {:?} but got {:?}", tag, tagged.tag);
  RuntimeError::fail(msg, "unwrapping type-tagged value")
}

pub fn wrap(tag: Inert<Tag>, value: int::Expr) -> Inert<Tagged> {
  Inert(Tagged { tag: tag.0, value })
}

pub fn resolve(
  protocol: Inert<Protocol>,
  value: ClauseInst,
) -> ExternResult<int::Expr> {
  let tag = value.request::<Tag>().ok_or_else(|| {
    let msg = format!("{value} is not type-tagged");
    RuntimeError::ext(msg, "resolving protocol impl")
  })?;
  get_impl(protocol, Inert(tag))
}

pub fn get_impl(
  Inert(proto): Inert<Protocol>,
  Inert(tag): Inert<Tag>,
) -> ExternResult<int::Expr> {
  if let Some(implem) = proto.0.impls.get(&tag.0.id) {
    Ok(implem.clone())
  } else if let Some(implem) = tag.0.impls.get(&proto.0.id) {
    Ok(implem.clone())
  } else {
    let message = format!("{tag:?} doesn't implement {proto:?}");
    RuntimeError::fail(message, "dispatching protocol")
  }
}

pub const fn gen_resolv(name: &'static str) -> impl GenClause {
  tpl::A(
    tpl::C("std::protocol::resolve"),
    tpl::V(Unstable::new(move |_| {
      refer_seq(name.split("::").chain(iter::once(TYPE_KEY)))
    })),
  )
}

pub fn protocol_lib() -> ConstTree {
  ConstTree::ns("std::protocol", [ConstTree::tree([
    xfn_ent("unwrap", [unwrap]),
    xfn_ent("wrap", [wrap]),
    xfn_ent("get_impl", [get_impl]),
    xfn_ent("resolve", [resolve]),
  ])])
}
