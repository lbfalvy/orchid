use orchid_base::interner::Tok;

use crate::tree::{OwnedItem, OwnedModule, OwnedTok, OwnedTokTree};

pub struct ParseCtx<'a> {
  tokens: &'a [OwnedTokTree]
}

pub fn split_br(ctx: ParseCtx) -> impl Iterator<Item = ParseCtx> {
  ctx.tokens.split(|t| matches!(t.tok, OwnedTok::BR)).map(|tokens| ParseCtx { tokens })
}

pub fn strip_br(tt: &OwnedTokTree) -> Option<OwnedTokTree> {
  let tok = match &tt.tok {
    OwnedTok::BR => return None,
    OwnedTok::Lambda(arg) => OwnedTok::Lambda(arg.iter().filter_map(strip_br).collect()),
    OwnedTok::S(p, b) => OwnedTok::S(p.clone(), b.iter().filter_map(strip_br).collect()),
    t => t.clone(),
  };
  Some(OwnedTokTree { tok, range: tt.range.clone() })
}

pub fn parse_items(ctx: ParseCtx) -> Vec<OwnedItem> {
  todo!()
}

pub fn parse_item(ctx: ParseCtx) -> OwnedItem {
  todo!()
}

pub fn parse_module(ctx: ParseCtx) -> (Tok<String>, OwnedModule) {
  todo!()
}