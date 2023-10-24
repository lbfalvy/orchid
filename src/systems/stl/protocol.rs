use std::fmt::Debug;
use std::sync::Arc;

use hashbrown::HashMap;
use itertools::Itertools;

use super::cross_pipeline::defer_to_runtime;
use super::reflect::RefEqual;
use crate::ast::{self, Constant, Expr, PType};
use crate::error::{ProjectResult, RuntimeError};
use crate::foreign::{xfn_2ary, Atomic, InertAtomic, XfnResult};
use crate::interpreted::ExprInst;
use crate::parse::errors::{Expected, ExpectedBlock, ExpectedName};
use crate::parse::{
  parse_entries, parse_exprv, parse_line, parse_nsname, split_lines,
  vec_to_single, Context, Lexeme, LineParser, LineParserOut, Stream,
};
use crate::sourcefile::{
  FileEntry, FileEntryKind, Member, MemberKind, ModuleBlock,
};
use crate::systems::parse_custom_line::custom_line;
use crate::utils::pure_seq::pushed;
use crate::{ConstTree, Interner, Location, Tok, VName};

pub struct TypeData {
  pub id: RefEqual,
  pub display_name: Tok<String>,
  pub impls: HashMap<RefEqual, ExprInst>,
}

#[derive(Clone)]
pub struct Protocol(pub Arc<TypeData>);
impl Debug for Protocol {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple(&self.0.display_name).field(&self.0.id.id()).finish()
  }
}
impl InertAtomic for Protocol {
  fn type_str() -> &'static str { "Protocol" }
}

#[derive(Clone)]
pub struct Tag(pub Arc<TypeData>);
impl Debug for Tag {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple(&self.0.display_name).field(&self.0.id.id()).finish()
  }
}
impl InertAtomic for Tag {
  fn type_str() -> &'static str { "Tag" }
  fn strict_eq(&self, other: &Self) -> bool { self.0.id == other.0.id }
}

#[derive(Clone)]
pub struct Tagged {
  pub tag: Tag,
  pub value: ExprInst,
}
impl Debug for Tagged {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_tuple("Tagged").field(&self.tag).field(&self.value).finish()
  }
}
impl InertAtomic for Tagged {
  fn type_str() -> &'static str { "Tagged" }
}

fn parse_impl(
  tail: Stream,
  ctx: &(impl Context + ?Sized),
) -> Option<ProjectResult<(VName, Expr<VName>)>> {
  custom_line(tail, ctx.interner().i("impl"), false).map(|res| {
    let (_, tail, _) = res?;
    let (name, tail) = parse_nsname(tail, ctx)?;
    let (walrus, tail) = tail.trim().pop()?;
    Expected::expect(Lexeme::Walrus, walrus)?;
    let (body, empty) = parse_exprv(tail, None, ctx)?;
    empty.expect_empty()?;
    let value = vec_to_single(tail.fallback, body)?;
    Ok((name, value))
  })
}

struct Impl {
  target: VName,
  value: Expr<VName>,
}

fn extract_impls(
  tail: Stream,
  ctx: &(impl Context + ?Sized),
  location: Location,
  typeid_name: Tok<String>,
) -> ProjectResult<(Vec<FileEntry>, Vec<Impl>)> {
  let mut lines = Vec::new();
  let mut impls = Vec::new(); // name1, value1, name2, value2, etc...
  for line in split_lines(tail) {
    match parse_impl(line, ctx) {
      Some(result) => {
        let (name, value) = result?;
        impls.push(Impl { target: pushed(name, typeid_name.clone()), value });
      },
      None => lines.extend(
        parse_line(line, ctx)?.into_iter().map(|k| k.wrap(location.clone())),
      ),
    }
  }
  Ok((lines, impls))
}

pub fn protocol_parser<'a>(
  tail: Stream<'_>,
  ctx: &'a (impl Context + ?Sized + 'a),
) -> LineParserOut {
  let i = ctx.interner();
  custom_line(tail, i.i("protocol"), true).map(|res| {
    let (exported, tail, line_loc) = res?;
    let (name, tail) = tail.pop()?;
    let name = ExpectedName::expect(name)?;
    let tail = ExpectedBlock::expect(tail, PType::Par)?;
    let protoid = RefEqual::new();
    let (lines, impls) =
      extract_impls(tail, ctx, line_loc.clone(), i.i("__type_id__"))?;
    let prelude = "
    import std::protocol
    const resolve := protocol::resolve __protocol__
    const get_impl := protocol::get_impl __protocol__
  ";
    let body = parse_entries(ctx, prelude, line_loc.clone())?
      .into_iter()
      .chain(
        [
          ("__protocol_id__", protoid.clone().ast_cls()),
          (
            "__protocol__",
            defer_to_runtime(
              impls.into_iter().flat_map(|Impl { target, value }| {
                [ast::Clause::Name(target).into_expr(), value]
                  .map(|e| ((), vec![e]))
              }),
              {
                let name = name.clone();
                move |pairs: Vec<((), ExprInst)>| {
                  let mut impls = HashMap::new();
                  debug_assert!(
                    pairs.len() % 2 == 0,
                    "names and values pair up"
                  );
                  let mut nvnvnv = pairs.into_iter().map(|t| t.1);
                  while let Some((name, value)) = nvnvnv.next_tuple() {
                    let key = name.downcast::<RefEqual>()?;
                    impls.insert(key, value);
                  }
                  let id = protoid.clone();
                  let display_name = name.clone();
                  Ok(Protocol(Arc::new(TypeData { id, display_name, impls })))
                }
              },
            ),
          ),
        ]
        .map(|(n, value)| {
          let value = Expr { value, location: line_loc.clone() };
          MemberKind::Constant(Constant { name: i.i(n), value })
            .to_entry(true, line_loc.clone())
        }),
      )
      .chain(lines)
      .collect();
    let kind = MemberKind::Module(ModuleBlock { name, body });
    Ok(vec![FileEntryKind::Member(Member { exported, kind })])
  })
}

pub fn type_parser(
  tail: Stream,
  ctx: &(impl Context + ?Sized),
) -> LineParserOut {
  let i = ctx.interner();
  custom_line(tail, ctx.interner().i("type"), true).map(|res| {
    let (exported, tail, line_loc) = res?;
    let (name, tail) = tail.pop()?;
    let name = ExpectedName::expect(name)?;
    let tail = ExpectedBlock::expect(tail, PType::Par)?;
    let typeid = RefEqual::new();
    let (lines, impls) =
      extract_impls(tail, ctx, line_loc.clone(), i.i("__protocol_id__"))?;
    let prelude = "
      import std::protocol
      const unwrap := protocol::unwrap __type_tag__
      const wrap := protocol::wrap __type_tag__
    ";
    let body = parse_entries(ctx, prelude, line_loc.clone())?
      .into_iter()
      .chain(
        [
          ("__type_id__", typeid.clone().ast_cls()),
          (
            "__type_tag__",
            defer_to_runtime(
              impls.into_iter().flat_map(|Impl { target, value }| {
                [ast::Clause::Name(target).into_expr(), value]
                  .map(|e| ((), vec![e]))
              }),
              {
                let name = name.clone();
                move |pairs: Vec<((), ExprInst)>| {
                  let mut impls = HashMap::new();
                  debug_assert!(
                    pairs.len() % 2 == 0,
                    "names and values pair up"
                  );
                  let mut nvnvnv = pairs.into_iter().map(|t| t.1);
                  while let Some((name, value)) = nvnvnv.next_tuple() {
                    let key = name.downcast::<RefEqual>()?;
                    impls.insert(key, value);
                  }
                  let id = typeid.clone();
                  let display_name = name.clone();
                  Ok(Tag(Arc::new(TypeData { id, display_name, impls })))
                }
              },
            ),
          ),
        ]
        .map(|(n, value)| {
          let value = Expr { value, location: line_loc.clone() };
          MemberKind::Constant(Constant { name: i.i(n), value })
            .to_entry(true, line_loc.clone())
        }),
      )
      .chain(lines)
      .collect();
    let kind = MemberKind::Module(ModuleBlock { name, body });
    Ok(vec![FileEntryKind::Member(Member { exported, kind })])
  })
}

pub fn parsers() -> Vec<Box<dyn LineParser>> {
  vec![
    Box::new(|tail, ctx| protocol_parser(tail, ctx)),
    Box::new(|tail, ctx| type_parser(tail, ctx)),
  ]
}

pub fn unwrap(tag: Tag, tagged: Tagged) -> XfnResult<ExprInst> {
  if tagged.tag.strict_eq(&tag) {
    return Ok(tagged.value);
  }
  let msg = format!("{:?} is not {:?}", tagged.tag, tag);
  RuntimeError::fail(msg, "unwrapping type-tagged value")
}

pub fn wrap(tag: Tag, value: ExprInst) -> XfnResult<Tagged> {
  Ok(Tagged { tag, value })
}

pub fn resolve(protocol: Protocol, tagged: Tagged) -> XfnResult<ExprInst> {
  get_impl(protocol, tagged.tag)
}

pub fn get_impl(proto: Protocol, tag: Tag) -> XfnResult<ExprInst> {
  if let Some(implem) = proto.0.impls.get(&tag.0.id) {
    return Ok(implem.clone());
  }
  if let Some(implem) = tag.0.impls.get(&proto.0.id) {
    return Ok(implem.clone());
  }
  let message = format!("{:?} doesn't implement {:?}", tag, proto);
  RuntimeError::fail(message, "dispatching protocol")
}

pub fn protocol_lib(i: &Interner) -> ConstTree {
  ConstTree::namespace(
    [i.i("protocol")],
    ConstTree::tree([
      (i.i("unwrap"), ConstTree::xfn(xfn_2ary(unwrap))),
      (i.i("wrap"), ConstTree::xfn(xfn_2ary(wrap))),
      (i.i("get_impl"), ConstTree::xfn(xfn_2ary(get_impl))),
      (i.i("resolve"), ConstTree::xfn(xfn_2ary(resolve))),
    ]),
  )
}
