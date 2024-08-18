use std::ops::{Deref, Range};
use std::sync::Arc;
use std::{fmt, iter};

use itertools::Itertools;

use crate::error::{mk_err, OrcRes, Reporter};
use crate::interner::{deintern, Tok};
use crate::location::Pos;
use crate::name::VPath;
use crate::tree::{AtomInTok, Paren, TokTree, Token};
use crate::{api, intern};

pub fn name_start(c: char) -> bool { c.is_alphabetic() || c == '_' }
pub fn name_char(c: char) -> bool { name_start(c) || c.is_numeric() }
pub fn op_char(c: char) -> bool { !name_char(c) && !c.is_whitespace() && !"()[]{}\\".contains(c) }
pub fn unrep_space(c: char) -> bool { c.is_whitespace() && !"\r\n".contains(c) }

#[derive(Debug)]
pub struct Snippet<'a, 'b, A: AtomInTok, X> {
  prev: &'a TokTree<'b, A, X>,
  cur: &'a [TokTree<'b, A, X>],
}
impl<'a, 'b, A: AtomInTok, X> Snippet<'a, 'b, A, X> {
  pub fn new(prev: &'a TokTree<'b, A, X>, cur: &'a [TokTree<'b, A, X>]) -> Self {
    Self { prev, cur }
  }
  pub fn split_at(self, pos: u32) -> (Self, Self) {
    let fst = Self { prev: self.prev, cur: &self.cur[..pos as usize] };
    let new_prev = if pos == 0 { self.prev } else { &self.cur[pos as usize - 1] };
    let snd = Self { prev: new_prev, cur: &self.cur[pos as usize..] };
    (fst, snd)
  }
  pub fn find_idx(self, mut f: impl FnMut(&Token<'b, A, X>) -> bool) -> Option<u32> {
    self.cur.iter().position(|t| f(&t.tok)).map(|t| t as u32)
  }
  pub fn get(self, idx: u32) -> Option<&'a TokTree<'b, A, X>> { self.cur.get(idx as usize) }
  pub fn len(self) -> u32 { self.cur.len() as u32 }
  pub fn prev(self) -> &'a TokTree<'b, A, X> { self.prev }
  pub fn pos(self) -> Range<u32> {
    (self.cur.first().map(|f| f.range.start..self.cur.last().unwrap().range.end))
      .unwrap_or(self.prev.range.clone())
  }
  pub fn pop_front(self) -> Option<(&'a TokTree<'b, A, X>, Self)> {
    self.cur.first().map(|r| (r, self.split_at(1).1))
  }
  pub fn split_once(self, f: impl FnMut(&Token<'b, A, X>) -> bool) -> Option<(Self, Self)> {
    let idx = self.find_idx(f)?;
    Some((self.split_at(idx).0, self.split_at(idx + 1).1))
  }
  pub fn split(
    mut self,
    mut f: impl FnMut(&Token<'b, A, X>) -> bool,
  ) -> impl Iterator<Item = Self> {
    iter::from_fn(move || {
      self.is_empty().then_some(())?;
      let (ret, next) = self.split_once(&mut f).unwrap_or(self.split_at(self.len()));
      self = next;
      Some(ret)
    })
  }
  pub fn is_empty(self) -> bool { self.len() == 0 }
  pub fn skip_fluff(self) -> Self {
    let non_fluff_start = self.find_idx(|t| !matches!(t, Token::NS | Token::Comment(_)));
    self.split_at(non_fluff_start.unwrap_or(self.len())).1
  }
}
impl<'a, 'b, A: AtomInTok, X> Copy for Snippet<'a, 'b, A, X> {}
impl<'a, 'b, A: AtomInTok, X> Clone for Snippet<'a, 'b, A, X> {
  fn clone(&self) -> Self { *self }
}
impl<'a, 'b, A: AtomInTok, X> Deref for Snippet<'a, 'b, A, X> {
  type Target = [TokTree<'b, A, X>];
  fn deref(&self) -> &Self::Target { self.cur }
}

/// Remove tokens that aren't meaningful in expression context, such as comments
/// or line breaks
pub fn strip_fluff<'a, A: AtomInTok, X: Clone>(
  tt: &TokTree<'a, A, X>,
) -> Option<TokTree<'a, A, X>> {
  let tok = match &tt.tok {
    Token::BR => return None,
    Token::Comment(_) => return None,
    Token::LambdaHead(arg) => Token::LambdaHead(arg.iter().filter_map(strip_fluff).collect()),
    Token::S(p, b) => Token::S(p.clone(), b.iter().filter_map(strip_fluff).collect()),
    t => t.clone(),
  };
  Some(TokTree { tok, range: tt.range.clone() })
}

#[derive(Clone, Debug)]
pub struct Comment {
  pub text: Arc<String>,
  pub pos: Pos,
}

pub fn line_items<'a, 'b, A: AtomInTok, X>(
  snip: Snippet<'a, 'b, A, X>,
) -> Vec<(Vec<Comment>, Snippet<'a, 'b, A, X>)> {
  let mut items = Vec::new();
  let mut comments = Vec::new();
  for mut line in snip.split(|t| matches!(t, Token::BR)) {
    match &line.cur {
      [TokTree { tok: Token::S(Paren::Round, tokens), .. }] => line.cur = tokens,
      [] => continue,
      _ => (),
    }
    match line.find_idx(|t| !matches!(t, Token::Comment(_))) {
      None => comments.extend(line.cur),
      Some(i) => {
        let (cmts, line) = line.split_at(i);
        let comments = Vec::from_iter(comments.drain(..).chain(cmts.cur).map(|t| match &t.tok {
          Token::Comment(c) => Comment { text: c.clone(), pos: Pos::Range(t.range.clone()) },
          _ => unreachable!("All are comments checked above"),
        }));
        items.push((comments, line));
      },
    }
  }
  items
}

pub fn try_pop_no_fluff<'a, 'b, A: AtomInTok, X>(
  snip: Snippet<'a, 'b, A, X>,
) -> OrcRes<(&'a TokTree<'b, A, X>, Snippet<'a, 'b, A, X>)> {
  snip.skip_fluff().pop_front().ok_or_else(|| {
    vec![mk_err(intern!(str: "Unexpected end"), "Pattern ends abruptly", [
      Pos::Range(snip.pos()).into()
    ])]
  })
}

pub fn expect_end(snip: Snippet<'_, '_, impl AtomInTok, impl Sized>) -> OrcRes<()> {
  match snip.skip_fluff().get(0) {
    Some(surplus) => Err(vec![mk_err(
      intern!(str: "Extra code after end of line"),
      "Code found after the end of the line",
      [Pos::Range(surplus.range.clone()).into()],
    )]),
    None => Ok(()),
  }
}

pub fn expect_tok<'a, 'b, A: AtomInTok, X: fmt::Display>(
  snip: Snippet<'a, 'b, A, X>, tok: Tok<String>
) -> OrcRes<Snippet<'a, 'b, A, X>> {
  let (head, tail) = try_pop_no_fluff(snip)?;
  match &head.tok {
    Token::Name(n) if *n == tok => Ok(tail),
    t => Err(vec![mk_err(
      intern!(str: "Expected specific keyword"), 
      format!("Expected {tok} but found {t}"),
      [Pos::Range(head.range.clone()).into()]
    )])
  }
}

pub fn parse_multiname<'a, 'b, A: AtomInTok, X: fmt::Display>(
  ctx: &impl Reporter,
  tail: Snippet<'a, 'b, A, X>,
) -> OrcRes<(Vec<CompName>, Snippet<'a, 'b, A, X>)> {
  let ret = rec(ctx, tail);
  #[allow(clippy::type_complexity)] // it's an internal function
  pub fn rec<'a, 'b, A: AtomInTok, X: fmt::Display>(
    ctx: &impl Reporter,
    tail: Snippet<'a, 'b, A, X>,
  ) -> OrcRes<(Vec<(Vec<Tok<String>>, Option<Tok<String>>, Pos)>, Snippet<'a, 'b, A, X>)> {
    let comma = intern!(str: ",");
    let globstar = intern!(str: "*");
    let (name, tail) = tail.skip_fluff().pop_front().ok_or_else(|| {
      vec![mk_err(
        intern!(str: "Expected name"),
        "Expected a name, a list of names, or a globstar.",
        [Pos::Range(tail.pos()).into()],
      )]
    })?;
    if let Some((Token::NS, tail)) = tail.skip_fluff().pop_front().map(|(tt, s)| (&tt.tok, s)) {
      let n = match &name.tok {
        Token::Name(n) if n.starts_with(name_start) => Ok(n),
        _ => Err(mk_err(intern!(str: "Unexpected name prefix"), "Only names can precede ::", [
          Pos::Range(name.range.clone()).into(),
        ])),
      };
      match (rec(ctx, tail), n) {
        (Err(ev), n) => Err(Vec::from_iter(ev.into_iter().chain(n.err()))),
        (Ok((_, tail)), Err(e)) => {
          ctx.report(e);
          Ok((vec![], tail))
        },
        (Ok((n, tail)), Ok(pre)) =>
          Ok((n.into_iter().update(|i| i.0.push(pre.clone())).collect_vec(), tail)),
      }
    } else {
      let names = match &name.tok {
        Token::Name(ntok) => {
          let nopt = match ntok {
            n if *n == globstar => None,
            n if n.starts_with(op_char) =>
              return Err(vec![mk_err(
                intern!(str: "Unescaped operator in multiname"),
                "Operators in multinames should be enclosed in []",
                [Pos::Range(name.range.clone()).into()],
              )]),
            n => Some(n.clone()),
          };
          vec![(vec![], nopt, Pos::Range(name.range.clone()))]
        },
        Token::S(Paren::Square, b) => {
          let mut ok = Vec::new();
          b.iter().for_each(|tt| match &tt.tok {
            Token::Name(n) if n.starts_with(op_char) =>
              ok.push((vec![], Some(n.clone()), Pos::Range(tt.range.clone()))),
            Token::BR | Token::Comment(_) => (),
            _ => ctx.report(mk_err(
              intern!(str: "Non-operator in escapement in multiname"),
              "In multinames, [] functions as a literal name list reserved for operators",
              [Pos::Range(name.range.clone()).into()],
            )),
          });
          ok
        },
        Token::S(Paren::Round, b) => {
          let mut ok = Vec::new();
          let body = Snippet::new(name, b);
          for csent in body.split(|n| matches!(n, Token::Name(n) if *n == comma)) {
            match rec(ctx, csent) {
              Err(e) => e.into_iter().for_each(|e| ctx.report(e)),
              Ok((v, surplus)) => match surplus.get(0) {
                None => ok.extend(v),
                Some(t) => ctx.report(mk_err(
                  intern!(str: "Unexpected token in multiname group"),
                  "Unexpected token. Likely missing a :: or , or wanted [] instead of ()",
                  [Pos::Range(t.range.clone()).into()],
                )),
              },
            }
          }
          ok
        },
        t =>
          return Err(vec![mk_err(
            intern!(str: "Unrecognized name end"),
            format!("Names cannot end with {t} tokens"),
            [Pos::Range(name.range.clone()).into()],
          )]),
      };
      Ok((names, tail))
    }
  }
  ret.map(|(i, tail)| {
    let i = Vec::from_iter((i.into_iter()).map(|(p, name, pos)| CompName {
      path: VPath::new(p.into_iter().rev()),
      name,
      pos,
    }));
    (i, tail)
  })
}

/// A compound name, possibly ending with a globstar
#[derive(Debug, Clone)]
pub struct CompName {
  pub path: VPath,
  pub name: Option<Tok<String>>,
  pub pos: Pos,
}
impl CompName {
  pub fn from_api(i: api::CompName) -> Self {
    Self {
      path: VPath::new(i.path.into_iter().map(deintern)),
      name: i.name.map(deintern),
      pos: Pos::from_api(&i.location),
    }
  }
}

#[cfg(test)]
mod test {
    use never::Never;

    use super::Snippet;

  fn _covary_snip_a<'a, 'b>(x: Snippet<'static, 'b, Never, ()>) -> Snippet<'a, 'b, Never, ()> { x }
  fn _covary_snip_b<'a, 'b>(x: Snippet<'a, 'static, Never, ()>) -> Snippet<'a, 'b, Never, ()> { x }
}