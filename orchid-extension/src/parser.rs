use orchid_base::error::OrcRes;
use orchid_base::parse::Snippet;

use crate::atom::{AtomFactory, ForeignAtom};
use crate::tree::GenTokTree;

pub type GenSnippet<'a> = Snippet<'a, 'a, ForeignAtom<'a>, AtomFactory>;

pub trait Parser: Send + Sync + Sized + Default + 'static {
  const LINE_HEAD: &'static str;
  fn parse(line: GenSnippet<'_>) -> OrcRes<Vec<GenTokTree<'_>>>;
}

pub trait DynParser: Send + Sync + 'static {
  fn line_head(&self) -> &'static str;
  fn parse<'a>(&self, line: GenSnippet<'a>) -> OrcRes<Vec<GenTokTree<'a>>>;
}

impl<T: Parser> DynParser for T {
  fn line_head(&self) -> &'static str { Self::LINE_HEAD }
  fn parse<'a>(&self, line: GenSnippet<'a>) -> OrcRes<Vec<GenTokTree<'a>>> { Self::parse(line) }
}

pub type ParserObj = &'static dyn DynParser;
