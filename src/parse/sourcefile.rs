use std::iter;
use std::rc::Rc;

use itertools::Itertools;

use super::context::Context;
use super::errors::{
  BadTokenInRegion, Expected, ExpectedName, GlobExport, LeadingNS,
  MisalignedParen, NamespacedExport, ReservedToken, UnexpectedEOL,
};
use super::lexer::Lexeme;
use super::multiname::parse_multiname;
use super::stream::Stream;
use super::Entry;
use crate::ast::{Clause, Constant, Expr, Rule};
use crate::error::{ProjectError, ProjectResult};
use crate::representations::location::Location;
use crate::representations::sourcefile::{FileEntry, Member, ModuleBlock};
use crate::representations::VName;
use crate::sourcefile::Import;
use crate::Primitive;

pub fn split_lines(module: Stream<'_>) -> impl Iterator<Item = Stream<'_>> {
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
          return Some(Stream::new(cur_prev, &module.data[begin..i]));
        },
        _ => (),
      }
    }
    // Include last line even without trailing newline
    if !finished {
      finished = true;
      return Some(Stream::new(fallback, &module.data[last_slice..]));
    }
    None
  })
}

pub fn parse_module_body(
  cursor: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<Vec<FileEntry>> {
  split_lines(cursor)
    .map(Stream::trim)
    .filter(|l| !l.data.is_empty())
    .map(|l| parse_line(l, ctx.clone()))
    .collect()
}

pub fn parse_line(
  cursor: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<FileEntry> {
  match cursor.get(0)?.lexeme {
    Lexeme::BR | Lexeme::Comment(_) => parse_line(cursor.step()?, ctx),
    Lexeme::Export => parse_export_line(cursor.step()?, ctx),
    Lexeme::Const | Lexeme::Macro | Lexeme::Module =>
      Ok(FileEntry::Internal(parse_member(cursor, ctx)?)),
    Lexeme::Import => {
      let (imports, cont) = parse_multiname(cursor.step()?, ctx)?;
      cont.expect_empty()?;
      Ok(FileEntry::Import(imports))
    },
    _ => {
      let err = BadTokenInRegion {
        entry: cursor.get(0)?.clone(),
        region: "start of line",
      };
      Err(err.rc())
    },
  }
}

pub fn parse_export_line(
  cursor: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<FileEntry> {
  let cursor = cursor.trim();
  match cursor.get(0)?.lexeme {
    Lexeme::NS => {
      let (names, cont) = parse_multiname(cursor.step()?, ctx)?;
      cont.expect_empty()?;
      let names = (names.into_iter())
        .map(|Import { name, path }| match (name, &path[..]) {
          (Some(n), []) => Ok(n),
          (None, _) => Err(GlobExport { location: cursor.location() }.rc()),
          _ => Err(NamespacedExport { location: cursor.location() }.rc()),
        })
        .collect::<Result<Vec<_>, _>>()?;
      Ok(FileEntry::Export(names))
    },
    Lexeme::Const | Lexeme::Macro | Lexeme::Module =>
      Ok(FileEntry::Exported(parse_member(cursor, ctx)?)),
    _ => {
      let err = BadTokenInRegion {
        entry: cursor.get(0)?.clone(),
        region: "exported line",
      };
      Err(err.rc())
    },
  }
}

fn parse_member(
  cursor: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<Member> {
  let (typemark, cursor) = cursor.trim().pop()?;
  match typemark.lexeme {
    Lexeme::Const => {
      let constant = parse_const(cursor, ctx)?;
      Ok(Member::Constant(constant))
    },
    Lexeme::Macro => {
      let rule = parse_rule(cursor, ctx)?;
      Ok(Member::Rule(rule))
    },
    Lexeme::Module => {
      let module = parse_module(cursor, ctx)?;
      Ok(Member::Module(module))
    },
    _ => {
      let err =
        BadTokenInRegion { entry: typemark.clone(), region: "member type" };
      Err(err.rc())
    },
  }
}

fn parse_rule(
  cursor: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<Rule<VName>> {
  let (pattern, prio, template) = cursor.find_map("arrow", |a| match a {
    Lexeme::Arrow(p) => Some(*p),
    _ => None,
  })?;
  let (pattern, _) = parse_exprv(pattern, None, ctx.clone())?;
  let (template, _) = parse_exprv(template, None, ctx)?;
  Ok(Rule { pattern, prio, template })
}

fn parse_const(
  cursor: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<Constant> {
  let (name_ent, cursor) = cursor.trim().pop()?;
  let name = ExpectedName::expect(name_ent)?;
  let (walrus_ent, cursor) = cursor.trim().pop()?;
  Expected::expect(Lexeme::Walrus, walrus_ent)?;
  let (body, _) = parse_exprv(cursor, None, ctx)?;
  Ok(Constant { name, value: vec_to_single(walrus_ent, body)? })
}

fn parse_module(
  cursor: Stream<'_>,
  ctx: impl Context,
) -> ProjectResult<ModuleBlock> {
  let (name_ent, cursor) = cursor.trim().pop()?;
  let name = ExpectedName::expect(name_ent)?;
  let (lp_ent, cursor) = cursor.trim().pop()?;
  Expected::expect(Lexeme::LP('('), lp_ent)?;
  let (last, cursor) = cursor.pop_back()?;
  Expected::expect(Lexeme::RP('('), last)?;
  let body = parse_module_body(cursor, ctx)?;
  Ok(ModuleBlock { name, body })
}

fn parse_exprv(
  mut cursor: Stream<'_>,
  paren: Option<char>,
  ctx: impl Context,
) -> ProjectResult<(Vec<Expr<VName>>, Stream<'_>)> {
  let mut output = Vec::new();
  cursor = cursor.trim();
  while let Ok(current) = cursor.get(0) {
    match &current.lexeme {
      Lexeme::BR | Lexeme::Comment(_) => unreachable!("Fillers skipped"),
      Lexeme::At | Lexeme::Type =>
        return Err(ReservedToken { entry: current.clone() }.rc()),
      Lexeme::Literal(l) => {
        output.push(Expr {
          value: Clause::P(Primitive::Literal(l.clone())),
          location: current.location(),
        });
        cursor = cursor.step()?;
      },
      Lexeme::Placeh(ph) => {
        output.push(Expr {
          value: Clause::Placeh(*ph),
          location: current.location(),
        });
        cursor = cursor.step()?;
      },
      Lexeme::Name(n) => {
        let location = cursor.location();
        let mut fullname = vec![*n];
        while cursor.get(1).ok().map(|e| &e.lexeme) == Some(&Lexeme::NS) {
          fullname.push(ExpectedName::expect(cursor.get(2)?)?);
          cursor = cursor.step()?.step()?;
        }
        output.push(Expr { value: Clause::Name(fullname), location });
        cursor = cursor.step()?;
      },
      Lexeme::NS =>
        return Err(LeadingNS { location: current.location() }.rc()),
      Lexeme::RP(c) =>
        return if Some(*c) == paren {
          Ok((output, cursor.step()?))
        } else {
          Err(MisalignedParen { entry: cursor.get(0)?.clone() }.rc())
        },
      Lexeme::LP(c) => {
        let (result, leftover) =
          parse_exprv(cursor.step()?, Some(*c), ctx.clone())?;
        output.push(Expr {
          value: Clause::S(*c, Rc::new(result)),
          location: cursor.get(0)?.location().to(leftover.fallback.location()),
        });
        cursor = leftover;
      },
      Lexeme::BS => {
        let (arg, body) =
          cursor.step()?.find("A '.'", |l| l == &Lexeme::Dot)?;
        let (arg, _) = parse_exprv(arg, None, ctx.clone())?;
        let (body, leftover) = parse_exprv(body, paren, ctx)?;
        output.push(Expr {
          location: cursor.location(),
          value: Clause::Lambda(Rc::new(arg), Rc::new(body)),
        });
        return Ok((output, leftover));
      },
      _ => {
        let err = BadTokenInRegion {
          entry: cursor.get(0)?.clone(),
          region: "expression",
        };
        return Err(err.rc());
      },
    }
    cursor = cursor.trim();
  }
  Ok((output, Stream::new(cursor.fallback, &[])))
}

fn vec_to_single(
  fallback: &Entry,
  v: Vec<Expr<VName>>,
) -> ProjectResult<Expr<VName>> {
  match v.len() {
    0 => return Err(UnexpectedEOL { entry: fallback.clone() }.rc()),
    1 => Ok(v.into_iter().exactly_one().unwrap()),
    _ => Ok(Expr {
      location: expr_slice_location(&v),
      value: Clause::S('(', Rc::new(v)),
    }),
  }
}

pub fn expr_slice_location(v: &[impl AsRef<Location>]) -> Location {
  v.first()
    .map(|l| l.as_ref().clone().to(v.last().unwrap().as_ref().clone()))
    .unwrap_or(Location::Unknown)
}
