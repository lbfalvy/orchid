use std::iter;
use std::rc::Rc;

use itertools::Itertools;

use super::context::Context;
use super::errors::{
  BadTokenInRegion, Expected, ExpectedBlock, ExpectedName, ExpectedSingleName,
  GlobExport, LeadingNS, MisalignedParen, NamespacedExport, ReservedToken,
  UnexpectedEOL,
};
use super::lexer::Lexeme;
use super::multiname::parse_multiname;
use super::stream::Stream;
use super::Entry;
use crate::ast::{Clause, Constant, Expr, PType, Rule};
use crate::error::{ProjectError, ProjectResult};
use crate::representations::location::Location;
use crate::representations::sourcefile::{FileEntry, MemberKind, ModuleBlock};
use crate::representations::VName;
use crate::sourcefile::{FileEntryKind, Import, Member};
use crate::utils::pure_seq::pushed;

/// Split the stream at each line break outside parentheses
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
  .map(Stream::trim)
  .map(|s| {
    s.pop()
      .and_then(|(first, inner)| {
        let (last, inner) = inner.pop_back()?;
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
  cursor: Stream<'_>,
  ctx: &(impl Context + ?Sized),
) -> ProjectResult<Vec<FileEntry>> {
  split_lines(cursor)
    .map(|l| {
      parse_line(l, ctx).map(move |kinds| {
        kinds
          .into_iter()
          .map(move |kind| FileEntry { locations: vec![l.location()], kind })
      })
    })
    .flatten_ok()
    .collect()
}

/// Parse a single, possibly exported entry
pub fn parse_line(
  cursor: Stream<'_>,
  ctx: &(impl Context + ?Sized),
) -> ProjectResult<Vec<FileEntryKind>> {
  for line_parser in ctx.line_parsers() {
    if let Some(result) = line_parser(cursor, &ctx) {
      return result;
    }
  }
  match &cursor.get(0)?.lexeme {
    Lexeme::BR | Lexeme::Comment(_) => parse_line(cursor.step()?, ctx),
    Lexeme::Name(n) if **n == "export" =>
      parse_export_line(cursor.step()?, ctx).map(|k| vec![k]),
    Lexeme::Name(n) if ["const", "macro", "module"].contains(&n.as_str()) =>
      Ok(vec![FileEntryKind::Member(Member {
        kind: parse_member(cursor, ctx)?,
        exported: false,
      })]),
    Lexeme::Name(n) if **n == "import" => {
      let (imports, cont) = parse_multiname(cursor.step()?, ctx)?;
      cont.expect_empty()?;
      Ok(vec![FileEntryKind::Import(imports)])
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

fn parse_export_line(
  cursor: Stream<'_>,
  ctx: &(impl Context + ?Sized),
) -> ProjectResult<FileEntryKind> {
  let cursor = cursor.trim();
  match &cursor.get(0)?.lexeme {
    Lexeme::NS => {
      let (names, cont) = parse_multiname(cursor.step()?, ctx)?;
      cont.expect_empty()?;
      let names = (names.into_iter())
        .map(|Import { name, path, location }| match (name, &path[..]) {
          (Some(n), []) => Ok((n, location)),
          (None, _) => Err(GlobExport(location).rc()),
          _ => Err(NamespacedExport(location).rc()),
        })
        .collect::<Result<Vec<_>, _>>()?;
      Ok(FileEntryKind::Export(names))
    },
    Lexeme::Name(n) if ["const", "macro", "module"].contains(&n.as_str()) =>
      Ok(FileEntryKind::Member(Member {
        kind: parse_member(cursor, ctx)?,
        exported: true,
      })),
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
  ctx: &(impl Context + ?Sized),
) -> ProjectResult<MemberKind> {
  let (typemark, cursor) = cursor.trim().pop()?;
  match &typemark.lexeme {
    Lexeme::Name(n) if **n == "const" => {
      let constant = parse_const(cursor, ctx)?;
      Ok(MemberKind::Constant(constant))
    },
    Lexeme::Name(n) if **n == "macro" => {
      let rule = parse_rule(cursor, &ctx)?;
      Ok(MemberKind::Rule(rule))
    },
    Lexeme::Name(n) if **n == "module" => {
      let module = parse_module(cursor, ctx)?;
      Ok(MemberKind::Module(module))
    },
    _ => {
      let err =
        BadTokenInRegion { entry: typemark.clone(), region: "member type" };
      Err(err.rc())
    },
  }
}

/// Parse a macro rule
pub fn parse_rule(
  cursor: Stream<'_>,
  ctx: &impl Context,
) -> ProjectResult<Rule<VName>> {
  let (pattern, prio, template) = cursor.find_map("arrow", |a| match a {
    Lexeme::Arrow(p) => Some(*p),
    _ => None,
  })?;
  let (pattern, _) = parse_exprv(pattern, None, ctx)?;
  let (template, _) = parse_exprv(template, None, ctx)?;
  Ok(Rule { pattern, prio, template })
}

/// Parse a constant declaration
pub fn parse_const(
  cursor: Stream<'_>,
  ctx: &(impl Context + ?Sized),
) -> ProjectResult<Constant> {
  let (name_ent, cursor) = cursor.trim().pop()?;
  let name = ExpectedName::expect(name_ent)?;
  let (walrus_ent, cursor) = cursor.trim().pop()?;
  Expected::expect(Lexeme::Walrus, walrus_ent)?;
  let (body, _) = parse_exprv(cursor, None, ctx)?;
  Ok(Constant { name, value: vec_to_single(walrus_ent, body)? })
}

/// Parse a namespaced name. TODO: use this for modules
pub fn parse_nsname<'a>(
  cursor: Stream<'a>,
  ctx: &(impl Context + ?Sized),
) -> ProjectResult<(VName, Stream<'a>)> {
  let (name, tail) = parse_multiname(cursor, ctx)?;
  let name = match name.into_iter().exactly_one() {
    Ok(Import { name: Some(name), path, .. }) => pushed(path, name),
    _ => {
      let loc = cursor.data[0].location().to(tail.data[0].location());
      return Err(ExpectedSingleName(loc).rc());
    },
  };
  Ok((name, tail))
}

/// Parse a submodule declaration
pub fn parse_module(
  cursor: Stream<'_>,
  ctx: &(impl Context + ?Sized),
) -> ProjectResult<ModuleBlock> {
  let (name_ent, cursor) = cursor.trim().pop()?;
  let name = ExpectedName::expect(name_ent)?;
  let body = ExpectedBlock::expect(cursor, PType::Par)?;
  Ok(ModuleBlock { name, body: parse_module_body(body, ctx)? })
}

/// Parse a sequence of expressions
pub fn parse_exprv<'a>(
  mut cursor: Stream<'a>,
  paren: Option<PType>,
  ctx: &(impl Context + ?Sized),
) -> ProjectResult<(Vec<Expr<VName>>, Stream<'a>)> {
  let mut output = Vec::new();
  cursor = cursor.trim();
  while let Ok(current) = cursor.get(0) {
    match &current.lexeme {
      Lexeme::BR | Lexeme::Comment(_) => unreachable!("Fillers skipped"),
      Lexeme::At | Lexeme::Type =>
        return Err(ReservedToken { entry: current.clone() }.rc()),
      Lexeme::Atom(a) => {
        let value = Clause::Atom(a.clone());
        output.push(Expr { value, location: current.location() });
        cursor = cursor.step()?;
      },
      Lexeme::Placeh(ph) => {
        output.push(Expr {
          value: Clause::Placeh(ph.clone()),
          location: current.location(),
        });
        cursor = cursor.step()?;
      },
      Lexeme::Name(n) => {
        let location = cursor.location();
        let mut fullname: VName = vec![n.clone()];
        while cursor.get(1).map_or(false, |e| e.lexeme.strict_eq(&Lexeme::NS)) {
          fullname.push(ExpectedName::expect(cursor.get(2)?)?);
          cursor = cursor.step()?.step()?;
        }
        output.push(Expr { value: Clause::Name(fullname), location });
        cursor = cursor.step()?;
      },
      Lexeme::NS => return Err(LeadingNS(current.location()).rc()),
      Lexeme::RP(c) =>
        return if Some(*c) == paren {
          Ok((output, cursor.step()?))
        } else {
          Err(MisalignedParen(cursor.get(0)?.clone()).rc())
        },
      Lexeme::LP(c) => {
        let (result, leftover) = parse_exprv(cursor.step()?, Some(*c), ctx)?;
        output.push(Expr {
          value: Clause::S(*c, Rc::new(result)),
          location: cursor.get(0)?.location().to(leftover.fallback.location()),
        });
        cursor = leftover;
      },
      Lexeme::BS => {
        let dot = ctx.interner().i(".");
        let (arg, body) = (cursor.step())?
          .find("A '.'", |l| l.strict_eq(&Lexeme::Name(dot.clone())))?;
        let (arg, _) = parse_exprv(arg, None, ctx)?;
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

/// Wrap an expression list in parentheses if necessary
pub fn vec_to_single(
  fallback: &Entry,
  v: Vec<Expr<VName>>,
) -> ProjectResult<Expr<VName>> {
  match v.len() {
    0 => Err(UnexpectedEOL { entry: fallback.clone() }.rc()),
    1 => Ok(v.into_iter().exactly_one().unwrap()),
    _ => Ok(Expr {
      location: expr_slice_location(&v),
      value: Clause::S(PType::Par, Rc::new(v)),
    }),
  }
}

/// Return the location of a sequence of consecutive expressions
#[must_use]
pub fn expr_slice_location(v: &[impl AsRef<Location>]) -> Location {
  v.first()
    .map(|l| l.as_ref().clone().to(v.last().unwrap().as_ref().clone()))
    .unwrap_or(Location::Unknown)
}
