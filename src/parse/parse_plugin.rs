//! Abstractions for dynamic extensions to the parser that act across entries.
//! Macros are the primary syntax extension  mechanism, but they only operate
//! within a constant and can't interfere with name reproject.

use std::ops::Range;

use dyn_clone::DynClone;
use intern_all::Tok;

use super::context::ParseCtx;
use super::errors::{expect, expect_block, expect_name};
use super::facade::parse_entries;
use super::frag::Frag;
use super::lexer::{Entry, Lexeme};
use super::parsed::{Constant, Expr, ModuleBlock, PType, Rule, SourceLine, SourceLineKind};
use super::sourcefile::{
  exprv_to_single, parse_const, parse_exprv, parse_line, parse_module, parse_module_body,
  parse_nsname, parse_rule, split_lines,
};
use crate::error::{ProjectErrorObj, ProjectResult};
use crate::location::SourceRange;
use crate::name::VName;
use crate::utils::boxed_iter::BoxedIter;

/// Information and actions exposed to [ParseLinePlugin]. A plugin should never
/// import and call the parser directly because it might be executed in a
/// different version of the parser.
pub trait ParsePluginReq<'t> {
  // ################ Frag and ParseCtx ################

  /// The token sequence this parser must parse
  fn frag(&self) -> Frag;
  /// Get the location of a fragment
  fn frag_loc(&self, f: Frag) -> SourceRange;
  /// Convert a numeric byte range into a location
  fn range_loc(&self, r: Range<usize>) -> SourceRange;
  /// Remove the first token of the fragment
  fn pop<'a>(&self, f: Frag<'a>) -> ProjectResult<(&'a Entry, Frag<'a>)>;
  /// Remove the last element of the fragment
  fn pop_back<'a>(&self, f: Frag<'a>) -> ProjectResult<(&'a Entry, Frag<'a>)>;

  // ################ Parser states ################

  /// Split up the lines in a fragment. The fragment must outlive the iterator
  /// and the request itself must outlive both
  fn split_lines<'a: 'b, 'b>(&'b self, f: Frag<'a>) -> BoxedIter<'b, Frag<'a>>
  where 't: 'b + 'a;
  /// Parse a sequence of source lines separated by line breaks
  fn parse_module_body(&self, frag: Frag) -> ProjectResult<Vec<SourceLine>>;
  /// Parse a single source line. This returns a vector because plugins can
  /// convert a single line into multiple entries
  fn parse_line(&self, frag: Frag) -> ProjectResult<Vec<SourceLineKind>>;
  /// Parse a macro rule `<exprv> =prio=> <exprv>`
  fn parse_rule(&self, frag: Frag) -> ProjectResult<Rule>;
  /// Parse a constant declaration `<name> := <exprv>`
  fn parse_const(&self, frag: Frag) -> ProjectResult<Constant>;
  /// Parse a namespaced name `name::name`
  fn parse_nsname<'a>(&self, f: Frag<'a>) -> ProjectResult<(VName, Frag<'a>)>;
  /// Parse a module declaration. `<name> ( <module_body> )`
  fn parse_module(&self, frag: Frag) -> ProjectResult<ModuleBlock>;
  /// Parse a sequence of expressions. In principle, it never makes sense to
  /// parse a single expression because it could always be a macro invocation.
  fn parse_exprv<'a>(&self, f: Frag<'a>, p: Option<PType>) -> ProjectResult<(Vec<Expr>, Frag<'a>)>;
  /// Parse a prepared string of code
  fn parse_entries(&self, t: &'static str, r: SourceRange) -> Vec<SourceLine>;
  /// Convert a sequence of expressions to a single one by parenthesization if
  /// necessary
  fn vec_to_single(&self, fallback: &Entry, v: Vec<Expr>) -> ProjectResult<Expr>;

  // ################ Assertions ################

  /// Unwrap a single name token or raise an error
  fn expect_name(&self, entry: &Entry) -> ProjectResult<Tok<String>>;
  /// Assert that the entry contains exactly the specified lexeme
  fn expect(&self, l: Lexeme, e: &Entry) -> ProjectResult<()>;
  /// Remove two parentheses from the ends of the cursor
  fn expect_block<'a>(&self, f: Frag<'a>, p: PType) -> ProjectResult<Frag<'a>>;
  /// Ensure that the fragment is empty
  fn expect_empty(&self, f: Frag) -> ProjectResult<()>;
  /// Report a fatal error while also producing output to be consumed by later
  /// stages for improved error reporting
  fn report_err(&self, e: ProjectErrorObj);
}

/// External plugin that parses an unrecognized source line into lines of
/// recognized types
pub trait ParseLinePlugin: Sync + Send + DynClone {
  /// Attempt to parse a line. Returns [None] if the line isn't recognized,
  /// [Some][Err] if it's recognized but incorrect.
  fn parse(&self, req: &dyn ParsePluginReq) -> Option<ProjectResult<Vec<SourceLineKind>>>;
}

/// Implementation of [ParsePluginReq] exposing sub-parsers and data to the
/// plugin via dynamic dispatch
pub struct ParsePlugReqImpl<'a, TCtx: ParseCtx + ?Sized> {
  /// Fragment of text to be parsed by the plugin
  pub frag: Frag<'a>,
  /// Context for recursive commands and to expose to the plugin
  pub ctx: &'a TCtx,
}
impl<'ty, TCtx: ParseCtx + ?Sized> ParsePluginReq<'ty> for ParsePlugReqImpl<'ty, TCtx> {
  fn frag(&self) -> Frag { self.frag }
  fn frag_loc(&self, f: Frag) -> SourceRange { self.range_loc(f.range()) }
  fn range_loc(&self, r: Range<usize>) -> SourceRange { self.ctx.range_loc(&r) }
  fn pop<'a>(&self, f: Frag<'a>) -> ProjectResult<(&'a Entry, Frag<'a>)> { f.pop(self.ctx) }
  fn pop_back<'a>(&self, f: Frag<'a>) -> ProjectResult<(&'a Entry, Frag<'a>)> {
    f.pop_back(self.ctx)
  }
  fn split_lines<'a: 'b, 'b>(&'b self, f: Frag<'a>) -> BoxedIter<'b, Frag<'a>>
  where
    'ty: 'b,
    'ty: 'a,
  {
    Box::new(split_lines(f, self.ctx))
  }
  fn parse_module_body(&self, f: Frag) -> ProjectResult<Vec<SourceLine>> {
    Ok(parse_module_body(f, self.ctx))
  }
  fn parse_line(&self, f: Frag) -> ProjectResult<Vec<SourceLineKind>> { parse_line(f, self.ctx) }
  fn parse_rule(&self, f: Frag) -> ProjectResult<Rule> { parse_rule(f, self.ctx) }
  fn parse_const(&self, f: Frag) -> ProjectResult<Constant> { parse_const(f, self.ctx) }
  fn parse_nsname<'a>(&self, f: Frag<'a>) -> ProjectResult<(VName, Frag<'a>)> {
    parse_nsname(f, self.ctx)
  }
  fn parse_module(&self, f: Frag) -> ProjectResult<ModuleBlock> { parse_module(f, self.ctx) }
  fn parse_exprv<'a>(&self, f: Frag<'a>, p: Option<PType>) -> ProjectResult<(Vec<Expr>, Frag<'a>)> {
    parse_exprv(f, p, self.ctx)
  }
  fn parse_entries(&self, s: &'static str, r: SourceRange) -> Vec<SourceLine> {
    parse_entries(&self.ctx, s, r)
  }
  fn vec_to_single(&self, fb: &Entry, v: Vec<Expr>) -> ProjectResult<Expr> {
    exprv_to_single(fb, v, self.ctx)
  }
  fn expect_name(&self, e: &Entry) -> ProjectResult<Tok<String>> { expect_name(e, self.ctx) }
  fn expect(&self, l: Lexeme, e: &Entry) -> ProjectResult<()> { expect(l, e, self.ctx) }
  fn expect_block<'a>(&self, f: Frag<'a>, t: PType) -> ProjectResult<Frag<'a>> {
    expect_block(f, t, self.ctx)
  }
  fn expect_empty(&self, f: Frag) -> ProjectResult<()> { f.expect_empty(self.ctx) }
  fn report_err(&self, e: ProjectErrorObj) { self.ctx.reporter().report(e) }
}
