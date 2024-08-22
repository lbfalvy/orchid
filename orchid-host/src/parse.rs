use std::{iter, thread};

use itertools::Itertools;
use never::Never;
use orchid_base::error::{mk_err, OrcErr, OrcRes, Reporter};
use orchid_base::intern;
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
use orchid_base::parse::{
  expect_end, line_items, parse_multiname, strip_fluff, try_pop_no_fluff, Comment, CompName,
  Snippet,
};
use orchid_base::tree::{Paren, TokTree, Token};

use crate::extension::{AtomHand, System};
use crate::tree::{Item, ItemKind, Member, MemberKind, Module, ParsTokTree};

type ParsSnippet<'a> = Snippet<'a, 'static, AtomHand, Never>;

pub trait ParseCtx: Send + Sync {
  fn systems(&self) -> impl Iterator<Item = &System>;
  fn reporter(&self) -> &impl Reporter;
}

pub fn parse_items(ctx: &impl ParseCtx, items: ParsSnippet) -> OrcRes<Vec<Item>> {
  let lines = line_items(items);
  let mut ok = iter::from_fn(|| None).take(lines.len()).collect_vec();
  thread::scope(|s| {
    let mut threads = Vec::new();
    for (slot, (cmts, item)) in ok.iter_mut().zip(lines.into_iter()) {
      threads.push(s.spawn(move || {
        *slot = Some(parse_item(ctx, cmts, item)?);
        Ok::<(), Vec<OrcErr>>(())
      }))
    }
    for t in threads {
      t.join().unwrap().err().into_iter().flatten().for_each(|e| ctx.reporter().report(e))
    }
  });
  Ok(ok.into_iter().flatten().flatten().collect_vec())
}

pub fn parse_item(
  ctx: &impl ParseCtx,
  comments: Vec<Comment>,
  item: ParsSnippet,
) -> OrcRes<Vec<Item>> {
  match item.pop_front() {
    Some((TokTree { tok: Token::Name(n), .. }, postdisc)) => match n {
      n if *n == intern!(str: "export") => match try_pop_no_fluff(postdisc)? {
        (TokTree { tok: Token::Name(n), .. }, postdisc) =>
          parse_item_2(ctx, comments, true, n.clone(), postdisc),
        (TokTree { tok: Token::NS, .. }, postdisc) => {
          let (exports, surplus) = parse_multiname(ctx.reporter(), postdisc)?;
          let mut ok = Vec::new();
          exports.into_iter().for_each(|e| match (&e.path.as_slice(), e.name) {
            ([], Some(n)) => ok.push(Item {
              comments: comments.clone(),
              pos: e.pos.clone(),
              kind: ItemKind::Export(n),
            }),
            (_, Some(_)) => ctx.reporter().report(mk_err(
              intern!(str: "Compound export"),
              "Cannot export compound names (names containing the :: separator)",
              [e.pos.into()],
            )),
            (_, None) => ctx.reporter().report(mk_err(
              intern!(str: "Wildcard export"),
              "Exports cannot contain the globstar *",
              [e.pos.into()],
            )),
          });
          expect_end(surplus)?;
          Ok(ok)
        },
        (bogus, _) => Err(vec![mk_err(
          intern!(str: "Malformed export"),
          "`export` can either prefix other lines or list names inside ::( ) or ::[ ]",
          [Pos::Range(bogus.range.clone()).into()],
        )]),
      },
      n if *n == intern!(str: "import") => parse_import(ctx, postdisc).map(|v| {
        Vec::from_iter(v.into_iter().map(|t| Item {
          comments: comments.clone(),
          pos: Pos::Range(postdisc.pos()),
          kind: ItemKind::Import(t),
        }))
      }),
      n => parse_item_2(ctx, comments, false, n.clone(), postdisc),
    },
    Some(_) => Err(vec![mk_err(
      intern!(str: "Expected a line type"),
      "All lines must begin with a keyword",
      [Pos::Range(item.pos()).into()],
    )]),
    None => unreachable!("These lines are filtered and aggregated in earlier stages"),
  }
}

pub fn parse_import(ctx: &impl ParseCtx, tail: ParsSnippet) -> OrcRes<Vec<CompName>> {
  let (imports, surplus) = parse_multiname(ctx.reporter(), tail)?;
  expect_end(surplus)?;
  Ok(imports)
}

pub fn parse_item_2(
  ctx: &impl ParseCtx,
  comments: Vec<Comment>,
  exported: bool,
  discr: Tok<String>,
  tail: ParsSnippet,
) -> OrcRes<Vec<Item>> {
  let kind = if discr == intern!(str: "mod") {
    let (name, body) = parse_module(ctx, tail)?;
    ItemKind::Member(Member::new(exported, name, MemberKind::Mod(body)))
  } else if discr == intern!(str: "const") {
    let (name, val) = parse_const(tail)?;
    ItemKind::Member(Member::new(exported, name, MemberKind::Const(val)))
  } else if let Some(sys) = ctx.systems().find(|s| s.can_parse(discr.clone())) {
    let line = sys.parse(tail.to_vec())?;
    return parse_items(ctx, Snippet::new(tail.prev(), &line));
  } else {
    let ext_lines = ctx.systems().flat_map(System::line_types).join(", ");
    return Err(vec![mk_err(
      intern!(str: "Unrecognized line type"),
      format!("Line types are: const, mod, macro, grammar, {ext_lines}"),
      [Pos::Range(tail.prev().range.clone()).into()],
    )]);
  };
  Ok(vec![Item { comments, pos: Pos::Range(tail.pos()), kind }])
}

pub fn parse_module(ctx: &impl ParseCtx, tail: ParsSnippet) -> OrcRes<(Tok<String>, Module)> {
  let (name, tail) = match try_pop_no_fluff(tail)? {
    (TokTree { tok: Token::Name(n), .. }, tail) => (n.clone(), tail),
    (tt, _) =>
      return Err(vec![mk_err(
        intern!(str: "Missing module name"),
        format!("A name was expected, {tt} was found"),
        [Pos::Range(tt.range.clone()).into()],
      )]),
  };
  let (body, surplus) = match try_pop_no_fluff(tail)? {
    (TokTree { tok: Token::S(Paren::Round, b), .. }, tail) => (b, tail),
    (tt, _) =>
      return Err(vec![mk_err(
        intern!(str: "Expected module body"),
        format!("A ( block ) was expected, {tt} was found"),
        [Pos::Range(tt.range.clone()).into()],
      )]),
  };
  let items = parse_items(ctx, ParsSnippet::new(surplus.prev(), body))?;
  Ok((name, Module { imports: vec![], items }))
}

pub fn parse_const(tail: ParsSnippet) -> OrcRes<(Tok<String>, Vec<ParsTokTree>)> {
  let (name, tail) = match try_pop_no_fluff(tail)? {
    (TokTree { tok: Token::Name(n), .. }, tail) => (n.clone(), tail),
    (tt, _) =>
      return Err(vec![mk_err(
        intern!(str: "Missing module name"),
        format!("A name was expected, {tt} was found"),
        [Pos::Range(tt.range.clone()).into()],
      )]),
  };
  let tail = match try_pop_no_fluff(tail)? {
    (TokTree { tok: Token::Name(n), .. }, tail) if *n == intern!(str: ":=") => tail,
    (tt, _) =>
      return Err(vec![mk_err(
        intern!(str: "Missing walrus := separator"),
        format!("Expected operator := , found {tt}"),
        [Pos::Range(tt.range.clone()).into()],
      )]),
  };
  try_pop_no_fluff(tail)?;
  Ok((name, tail.iter().flat_map(strip_fluff).collect_vec()))
}
