use std::iter;
use std::rc::Rc;

use intern_all::i;
use itertools::Itertools;

use super::context::ParseCtx;
use super::errors::{
  expect, expect_block, expect_name, BadTokenInRegion, ExpectedSingleName,
  GlobExport, LeadingNS, MisalignedParen, NamespacedExport, ParseErrorKind,
  ReservedToken, UnexpectedEOL,
};
use super::frag::Frag;
use super::lexer::{Entry, Lexeme};
use super::multiname::parse_multiname;
use super::parse_plugin::ParsePlugReqImpl;
use crate::error::ProjectResult;
use crate::name::VName;
use crate::parse::parsed::{
  Clause, Constant, Expr, Import, Member, MemberKind, ModuleBlock, PType, Rule,
  SourceLine, SourceLineKind,
};

/// Split the fragment at each line break outside parentheses
pub fn split_lines<'a>(
  module: Frag<'a>,
  ctx: &'a (impl ParseCtx + ?Sized),
) -> impl Iterator<Item = Frag<'a>> {
  let mut source = module.data.iter().enumerate();
  let mut fallback = module.fallback;
  let mut last_slice = 0;
  let mut finished = false;
  iter::from_fn(move || {
    let mut paren_count = 0;
    for (i, Entry { lexeme, .. }) in source.by_ref() {
      match lexeme {
        Lexeme::LP(_) => paren_count += 1,
        Lexeme::RP(_) => paren_count -= 1,
        Lexeme::BR if paren_count == 0 => {
          let begin = last_slice;
          last_slice = i + 1;
          let cur_prev = fallback;
          fallback = &module.data[i];
          return Some(Frag::new(cur_prev, &module.data[begin..i]));
        },
        _ => (),
      }
    }
    // Include last line even without trailing newline
    if !finished {
      finished = true;
      return Some(Frag::new(fallback, &module.data[last_slice..]));
    }
    None
  })
  .map(Frag::trim)
  .map(|s| {
    s.pop(ctx)
      .and_then(|(first, inner)| {
        let (last, inner) = inner.pop_back(ctx)?;
        match (&first.lexeme, &last.lexeme) {
          (Lexeme::LP(PType::Par), Lexeme::RP(PType::Par)) => Ok(inner.trim()),
          _ => Ok(s),
        }
      })
      .unwrap_or(s)
  })
  .filter(|l| !l.data.is_empty())
}

/// Parse linebreak-separated entries
pub fn parse_module_body(
  cursor: Frag<'_>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<Vec<SourceLine>> {
  let mut lines = Vec::new();
  for l in split_lines(cursor, ctx) {
    let kinds = parse_line(l, ctx)?;
    let r = ctx.range_loc(&l.range());
    lines.extend(
      kinds.into_iter().map(|kind| SourceLine { range: r.clone(), kind }),
    );
  }
  Ok(lines)
}

/// Parse a single, possibly exported entry
pub fn parse_line(
  cursor: Frag<'_>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<Vec<SourceLineKind>> {
  let req = ParsePlugReqImpl { ctx, frag: cursor };
  for line_parser in ctx.line_parsers() {
    if let Some(result) = line_parser.parse(&req) {
      return result;
    }
  }
  let head = cursor.get(0, ctx)?;
  match &head.lexeme {
    Lexeme::Comment(cmt) =>
      cmt.strip_prefix('|').and_then(|c| c.strip_suffix('|')).map_or_else(
        || parse_line(cursor.step(ctx)?, ctx),
        |cmt| Ok(vec![SourceLineKind::Comment(cmt.to_string())]),
      ),
    Lexeme::BR => parse_line(cursor.step(ctx)?, ctx),
    Lexeme::Name(n) if **n == "export" =>
      parse_export_line(cursor.step(ctx)?, ctx).map(|k| vec![k]),
    Lexeme::Name(n) if ["const", "macro", "module"].contains(&n.as_str()) => {
      let member = Member { exported: false, kind: parse_member(cursor, ctx)? };
      Ok(vec![SourceLineKind::Member(member)])
    },
    Lexeme::Name(n) if **n == "import" => {
      let (imports, cont) = parse_multiname(cursor.step(ctx)?, ctx)?;
      cont.expect_empty(ctx)?;
      Ok(vec![SourceLineKind::Import(imports)])
    },
    lexeme => {
      let lexeme = lexeme.clone();
      Err(
        BadTokenInRegion { lexeme, region: "start of line" }
          .pack(ctx.range_loc(&head.range)),
      )
    },
  }
}

fn parse_export_line(
  cursor: Frag<'_>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<SourceLineKind> {
  let cursor = cursor.trim();
  let head = cursor.get(0, ctx)?;
  match &head.lexeme {
    Lexeme::NS => {
      let (names, cont) = parse_multiname(cursor.step(ctx)?, ctx)?;
      cont.expect_empty(ctx)?;
      let names = (names.into_iter())
        .map(|Import { name, path, range }| match (name, &path[..]) {
          (Some(n), []) => Ok((n, range)),
          (None, _) => Err(GlobExport.pack(range)),
          _ => Err(NamespacedExport.pack(range)),
        })
        .collect::<Result<Vec<_>, _>>()?;
      Ok(SourceLineKind::Export(names))
    },
    Lexeme::Name(n) if ["const", "macro", "module"].contains(&n.as_str()) =>
      Ok(SourceLineKind::Member(Member {
        kind: parse_member(cursor, ctx)?,
        exported: true,
      })),
    lexeme => {
      let lexeme = lexeme.clone();
      let err = BadTokenInRegion { lexeme, region: "exported line" };
      Err(err.pack(ctx.range_loc(&head.range)))
    },
  }
}

fn parse_member(
  cursor: Frag<'_>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<MemberKind> {
  let (typemark, cursor) = cursor.trim().pop(ctx)?;
  match &typemark.lexeme {
    Lexeme::Name(n) if **n == "const" => {
      let constant = parse_const(cursor, ctx)?;
      Ok(MemberKind::Constant(constant))
    },
    Lexeme::Name(n) if **n == "macro" => {
      let rule = parse_rule(cursor, ctx)?;
      Ok(MemberKind::Rule(rule))
    },
    Lexeme::Name(n) if **n == "module" => {
      let module = parse_module(cursor, ctx)?;
      Ok(MemberKind::Module(module))
    },
    lexeme => {
      let lexeme = lexeme.clone();
      let err = BadTokenInRegion { lexeme, region: "member type" };
      Err(err.pack(ctx.range_loc(&typemark.range)))
    },
  }
}

/// Parse a macro rule
pub fn parse_rule(
  cursor: Frag<'_>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<Rule> {
  let (pattern, prio, template) =
    cursor.find_map("arrow", ctx, |a| match a {
      Lexeme::Arrow(p) => Some(*p),
      _ => None,
    })?;
  let (pattern, _) = parse_exprv(pattern, None, ctx)?;
  let (template, _) = parse_exprv(template, None, ctx)?;
  Ok(Rule { pattern, prio, template })
}

/// Parse a constant declaration
pub fn parse_const(
  cursor: Frag<'_>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<Constant> {
  let (name_ent, cursor) = cursor.trim().pop(ctx)?;
  let name = expect_name(name_ent, ctx)?;
  let (walrus_ent, cursor) = cursor.trim().pop(ctx)?;
  expect(Lexeme::Walrus, walrus_ent, ctx)?;
  let (body, _) = parse_exprv(cursor, None, ctx)?;
  Ok(Constant { name, value: vec_to_single(walrus_ent, body, ctx)? })
}

/// Parse a namespaced name. TODO: use this for modules
pub fn parse_nsname<'a>(
  cursor: Frag<'a>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<(VName, Frag<'a>)> {
  let (name, tail) = parse_multiname(cursor, ctx)?;
  match name.into_iter().exactly_one() {
    Ok(Import { name: Some(name), path, .. }) =>
      Ok((VName::new([name]).unwrap().prefix(path), tail)),
    Err(_) | Ok(Import { name: None, .. }) => {
      let range = cursor.data[0].range.start..tail.data[0].range.end;
      Err(ExpectedSingleName.pack(ctx.range_loc(&range)))
    },
  }
}

/// Parse a submodule declaration
pub fn parse_module(
  cursor: Frag<'_>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<ModuleBlock> {
  let (name_ent, cursor) = cursor.trim().pop(ctx)?;
  let name = expect_name(name_ent, ctx)?;
  let body = expect_block(cursor, PType::Par, ctx)?;
  Ok(ModuleBlock { name, body: parse_module_body(body, ctx)? })
}

/// Parse a sequence of expressions
pub fn parse_exprv<'a>(
  mut cursor: Frag<'a>,
  paren: Option<PType>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<(Vec<Expr>, Frag<'a>)> {
  let mut output = Vec::new();
  cursor = cursor.trim();
  while let Ok(current) = cursor.get(0, ctx) {
    match &current.lexeme {
      Lexeme::BR | Lexeme::Comment(_) => unreachable!("Fillers skipped"),
      Lexeme::At | Lexeme::Type => {
        let err = ReservedToken(current.lexeme.clone());
        return Err(err.pack(ctx.range_loc(&current.range)));
      },
      Lexeme::Atom(a) => {
        let value = Clause::Atom(a.clone());
        output.push(Expr { value, range: ctx.range_loc(&current.range) });
        cursor = cursor.step(ctx)?;
      },
      Lexeme::Placeh(ph) => {
        output.push(Expr {
          value: Clause::Placeh(ph.clone()),
          range: ctx.range_loc(&current.range),
        });
        cursor = cursor.step(ctx)?;
      },
      Lexeme::Name(n) => {
        let range = ctx.range_loc(&cursor.range());
        let mut fullname = VName::new([n.clone()]).unwrap();
        while cursor.get(1, ctx).is_ok_and(|e| e.lexeme.strict_eq(&Lexeme::NS))
        {
          fullname = fullname.suffix([expect_name(cursor.get(2, ctx)?, ctx)?]);
          cursor = cursor.step(ctx)?.step(ctx)?;
        }
        let clause = Clause::Name(fullname.to_sym());
        output.push(Expr { value: clause, range });
        cursor = cursor.step(ctx)?;
      },
      Lexeme::NS => return Err(LeadingNS.pack(ctx.range_loc(&current.range))),
      Lexeme::RP(c) => match paren {
        Some(exp_c) if exp_c == *c => return Ok((output, cursor.step(ctx)?)),
        _ => {
          let err = MisalignedParen(current.lexeme.clone());
          return Err(err.pack(ctx.range_loc(&current.range)));
        },
      },
      Lexeme::LP(c) => {
        let (result, leftover) = parse_exprv(cursor.step(ctx)?, Some(*c), ctx)?;
        let range = current.range.start..leftover.fallback.range.end;
        let value = Clause::S(*c, Rc::new(result));
        output.push(Expr { value, range: ctx.range_loc(&range) });
        cursor = leftover;
      },
      Lexeme::BS => {
        let dot = i(".");
        let (arg, body) = (cursor.step(ctx))?
          .find("A '.'", ctx, |l| l.strict_eq(&Lexeme::Name(dot.clone())))?;
        let (arg, _) = parse_exprv(arg, None, ctx)?;
        let (body, leftover) = parse_exprv(body, paren, ctx)?;
        output.push(Expr {
          range: ctx.range_loc(&cursor.range()),
          value: Clause::Lambda(Rc::new(arg), Rc::new(body)),
        });
        return Ok((output, leftover));
      },
      lexeme => {
        let lexeme = lexeme.clone();
        let err = BadTokenInRegion { lexeme, region: "expression" };
        return Err(err.pack(ctx.range_loc(&current.range)));
      },
    }
    cursor = cursor.trim();
  }
  Ok((output, Frag::new(cursor.fallback, &[])))
}

/// Wrap an expression list in parentheses if necessary
pub fn vec_to_single(
  fallback: &Entry,
  v: Vec<Expr>,
  ctx: &(impl ParseCtx + ?Sized),
) -> ProjectResult<Expr> {
  match v.len() {
    0 => {
      let err = UnexpectedEOL(fallback.lexeme.clone());
      Err(err.pack(ctx.range_loc(&fallback.range)))
    },
    1 => Ok(v.into_iter().exactly_one().unwrap()),
    _ => {
      let f_range = &v.first().unwrap().range;
      let l_range = &v.last().unwrap().range;
      let range = f_range.map_range(|r| r.start..l_range.range.end);
      Ok(Expr { range, value: Clause::S(PType::Par, Rc::new(v)) })
    },
  }
}
