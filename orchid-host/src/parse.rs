use std::{iter, thread};

use itertools::Itertools;
use never::Never;
use orchid_base::error::{mk_err, mk_errv, OrcErrv, OrcRes, Reporter};
use orchid_base::intern;
use orchid_base::interner::Tok;
use orchid_base::location::Pos;
use orchid_base::macros::{MTok, MTree};
use orchid_base::name::Sym;
use orchid_base::parse::{
  expect_end, line_items, parse_multiname, strip_fluff, try_pop_no_fluff, Comment, Import,
  Parsed, Snippet,
};
use orchid_base::tree::{Paren, TokTree, Token};
use substack::Substack;

use crate::extension::{AtomHand, System};
use crate::tree::{Code, CodeLocator, Item, ItemKind, Member, MemberKind, Module, ParsTokTree, Rule, RuleKind};

type ParsSnippet<'a> = Snippet<'a, 'static, AtomHand, Never>;

pub trait ParseCtx: Send + Sync {
  fn systems(&self) -> impl Iterator<Item = &System>;
  fn reporter(&self) -> &impl Reporter;
}

pub fn parse_items(
  ctx: &impl ParseCtx,
  path: Substack<Tok<String>>,
  items: ParsSnippet
) -> OrcRes<Vec<Item>> {
  let lines = line_items(items);
  let mut ok = iter::from_fn(|| None).take(lines.len()).collect_vec();
  thread::scope(|s| {
    let mut threads = Vec::new();
    for (slot, Parsed { output: cmts, tail }) in ok.iter_mut().zip(lines.into_iter()) {
      let path = &path;
      threads.push(s.spawn(move || {
        *slot = Some(parse_item(ctx, path.clone(), cmts, tail)?);
        Ok::<(), OrcErrv>(())
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
  path: Substack<Tok<String>>,
  comments: Vec<Comment>,
  item: ParsSnippet,
) -> OrcRes<Vec<Item>> {
  match item.pop_front() {
    Some((TokTree { tok: Token::Name(n), .. }, postdisc)) => match n {
      n if *n == intern!(str: "export") => match try_pop_no_fluff(postdisc)? {
        Parsed { output: TokTree { tok: Token::Name(n), .. }, tail } =>
          parse_exportable_item(ctx, path, comments, true, n.clone(), tail),
        Parsed { output: TokTree { tok: Token::NS, .. }, tail } => {
          let Parsed { output: exports, tail } = parse_multiname(ctx.reporter(), tail)?;
          let mut ok = Vec::new();
          exports.into_iter().for_each(|(e, pos)| match (&e.path.as_slice(), e.name) {
            ([], Some(n)) =>
              ok.push(Item { comments: comments.clone(), pos, kind: ItemKind::Export(n) }),
            (_, Some(_)) => ctx.reporter().report(mk_err(
              intern!(str: "Compound export"),
              "Cannot export compound names (names containing the :: separator)",
              [pos.into()],
            )),
            (_, None) => ctx.reporter().report(mk_err(
              intern!(str: "Wildcard export"),
              "Exports cannot contain the globstar *",
              [pos.into()],
            )),
          });
          expect_end(tail)?;
          Ok(ok)
        },
        Parsed { output, .. } => Err(mk_errv(
          intern!(str: "Malformed export"),
          "`export` can either prefix other lines or list names inside ::( ) or ::[ ]",
          [Pos::Range(output.range.clone()).into()],
        )),
      },
      n if *n == intern!(str: "import") => parse_import(ctx, postdisc).map(|v| {
        Vec::from_iter(v.into_iter().map(|(t, pos)| Item {
          comments: comments.clone(),
          pos,
          kind: ItemKind::Import(t),
        }))
      }),
      n => parse_exportable_item(ctx, path, comments, false, n.clone(), postdisc),
    },
    Some(_) =>
      Err(mk_errv(intern!(str: "Expected a line type"), "All lines must begin with a keyword", [
        Pos::Range(item.pos()).into(),
      ])),
    None => unreachable!("These lines are filtered and aggregated in earlier stages"),
  }
}

pub fn parse_import(ctx: &impl ParseCtx, tail: ParsSnippet) -> OrcRes<Vec<(Import, Pos)>> {
  let Parsed { output: imports, tail } = parse_multiname(ctx.reporter(), tail)?;
  expect_end(tail)?;
  Ok(imports)
}

pub fn parse_exportable_item(
  ctx: &impl ParseCtx,
  path: Substack<Tok<String>>,
  comments: Vec<Comment>,
  exported: bool,
  discr: Tok<String>,
  tail: ParsSnippet,
) -> OrcRes<Vec<Item>> {
  let kind = if discr == intern!(str: "mod") {
    let (name, body) = parse_module(ctx, path, tail)?;
    ItemKind::Member(Member::new(name, MemberKind::Mod(body)))
  } else if discr == intern!(str: "const") {
    let (name, val) = parse_const(tail)?;
    let locator = CodeLocator::to_const(path.push(name.clone()).unreverse());
    ItemKind::Member(Member::new(name, MemberKind::Const(Code::from_code(locator, val))))
  } else if let Some(sys) = ctx.systems().find(|s| s.can_parse(discr.clone())) {
    let line = sys.parse(tail.to_vec(), exported, comments)?;
    return parse_items(ctx, path, Snippet::new(tail.prev(), &line));
  } else {
    let ext_lines = ctx.systems().flat_map(System::line_types).join(", ");
    return Err(mk_errv(
      intern!(str: "Unrecognized line type"),
      format!("Line types are: const, mod, macro, grammar, {ext_lines}"),
      [Pos::Range(tail.prev().range.clone()).into()],
    ));
  };
  Ok(vec![Item { comments, pos: Pos::Range(tail.pos()), kind }])
}

pub fn parse_module(
  ctx: &impl ParseCtx,
  path: Substack<Tok<String>>,
  tail: ParsSnippet
) -> OrcRes<(Tok<String>, Module)> {
  let (name, tail) = match try_pop_no_fluff(tail)? {
    Parsed { output: TokTree { tok: Token::Name(n), .. }, tail } => (n.clone(), tail),
    Parsed { output, .. } =>
      return Err(mk_errv(
        intern!(str: "Missing module name"),
        format!("A name was expected, {output} was found"),
        [Pos::Range(output.range.clone()).into()],
      )),
  };
  let Parsed { output, tail: surplus } = try_pop_no_fluff(tail)?;
  expect_end(surplus)?;
  let body = output.as_s(Paren::Round).ok_or_else(|| mk_errv(
    intern!(str: "Expected module body"),
    format!("A ( block ) was expected, {output} was found"),
    [Pos::Range(output.range.clone()).into()],
  ))?;
  let path = path.push(name.clone());
  Ok((name, Module::new(parse_items(ctx, path, body)?)))
}

pub fn parse_const(tail: ParsSnippet) -> OrcRes<(Tok<String>, Vec<ParsTokTree>)> {
  let Parsed { output, tail } = try_pop_no_fluff(tail)?;
  let name = output.as_name().ok_or_else(|| mk_errv(
    intern!(str: "Missing module name"),
    format!("A name was expected, {output} was found"),
    [Pos::Range(output.range.clone()).into()],
  ))?;
  let Parsed { output, tail } = try_pop_no_fluff(tail)?;
  if !output.is_kw(intern!(str: "=")) {
    return Err(mk_errv(
      intern!(str: "Missing walrus := separator"),
      format!("Expected operator := , found {output}"),
      [Pos::Range(output.range.clone()).into()],
    ))
  }
  try_pop_no_fluff(tail)?;
  Ok((name, tail.iter().flat_map(strip_fluff).collect_vec()))
}

pub fn parse_mtree<'a>(
  mut snip: ParsSnippet<'a>
) -> OrcRes<Vec<MTree<'static>>> {
  let mut mtreev = Vec::new();
  while let Some((ttree, tail)) = snip.pop_front() {
    let (range, tok, tail) = match &ttree.tok {
      Token::S(p, b) => (
        ttree.range.clone(),
        MTok::S(*p, parse_mtree(Snippet::new(ttree, b))?),
        tail,
      ),
      Token::Name(tok) => {
        let mut segments = vec![tok.clone()];
        let mut end = ttree.range.end;
        while let Some((TokTree { tok: Token::NS, .. }, tail)) = snip.pop_front() {
          let Parsed { output, tail } = try_pop_no_fluff(tail)?;
          segments.push(output.as_name().ok_or_else(|| mk_errv(
            intern!(str: "Namespaced name interrupted"),
            "In expression context, :: must always be followed by a name.\n\
            ::() is permitted only in import and export items", 
            [Pos::Range(output.range.clone()).into()]
          ))?);
          snip = tail;
          end = output.range.end;
        }
        (ttree.range.start..end, MTok::Name(Sym::new(segments).unwrap()), snip)
      },
      Token::NS => return Err(mk_errv(
        intern!(str: "Unexpected :: in macro pattern"),
        ":: can only follow a name outside export statements",
        [Pos::Range(ttree.range.clone()).into()]
      )),
      Token::Ph(ph) => (ttree.range.clone(), MTok::Ph(ph.clone()), tail),
      Token::Atom(_) | Token::Macro(_) => return Err(mk_errv(
        intern!(str: "Unsupported token in macro patterns"), 
        format!("Macro patterns can only contain names, braces, and lambda, not {ttree}."),
        [Pos::Range(ttree.range.clone()).into()]
      )),
      Token::BR | Token::Comment(_) => continue,
      Token::Bottom(e) => return Err(e.clone()),
      Token::Lambda(arg, body) => {
        let tok = MTok::Lambda(
          parse_mtree(Snippet::new(&ttree, &arg))?,
          parse_mtree(Snippet::new(&ttree, &body))?,
        );
        (ttree.range.clone(), tok, tail)
      },
      Token::LambdaHead(arg) => (
        ttree.range.start..snip.pos().end,
        MTok::Lambda(parse_mtree(Snippet::new(&ttree, &arg))?, parse_mtree(tail)?),
        Snippet::new(ttree, &[]),
      ),
      Token::Slot(_) | Token::X(_) => panic!("Did not expect {} in parsed token tree", &ttree.tok),
    };
    mtreev.push(MTree { pos: Pos::Range(range.clone()), tok });
    snip = tail;
  }
  Ok(mtreev)
}

pub fn parse_macro(tail: ParsSnippet, macro_i: u16, path: Substack<Tok<String>>) -> OrcRes<Vec<Rule>> {
  let (surplus, prev, block) = match try_pop_no_fluff(tail)? {
    Parsed { tail, output: o@TokTree { tok: Token::S(Paren::Round, b), .. } } => (tail, o, b),
    Parsed { output, .. } => return Err(mk_errv(
      intern!(str: "m"),
      format!("Macro blocks must either start with a block or a ..$:number"), 
      [Pos::Range(output.range.clone()).into()]
    )),
  };
  expect_end(surplus)?;
  let mut errors = Vec::new();
  let mut rules = Vec::new();
  for (i, item) in line_items(Snippet::new(prev, &block)).into_iter().enumerate() {
    let Parsed { tail, output } = try_pop_no_fluff(item.tail)?;
    if !output.is_kw(intern!(str: "rule")) {
      errors.extend(mk_errv(
        intern!(str: "non-rule in macro"), 
        format!("Expected `rule`, got {output}"),
        [Pos::Range(output.range.clone()).into()]
      ));
      continue
    };
    let (pat, body) = match tail.split_once(|t| t.is_kw(intern!(str: "=>"))) {
      Some((a, b)) => (a, b),
      None => {
        errors.extend(mk_errv(
          intern!(str: "no => in macro rule"),
          "The pattern and body of a rule must be separated by a =>",
          [Pos::Range(tail.pos()).into()],
        ));
        continue
      }
    };
    rules.push(Rule {
      comments: item.output,
      pos: Pos::Range(tail.pos()),
      pattern: parse_mtree(pat)?,
      kind: RuleKind::Native(Code::from_code(
        CodeLocator::to_rule(path.unreverse(), macro_i, i as u16),
        body.to_vec(),
      ))
    })
  }
  if let Ok(e) = OrcErrv::new(errors) { Err(e) } else { Ok(rules) }
}
