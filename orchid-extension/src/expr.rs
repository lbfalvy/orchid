use orchid_api::atom::Atom;
use orchid_api::expr::ExprTicket;
use orchid_api::system::SysId;
use orchid_base::id_store::IdStore;
use orchid_base::intern::Token;

use crate::atom::{encode_atom_nodrop, DynOwnedAtom, OwnedAtom, ThinAtom, OBJ_STORE};
use crate::system::DynSystem;

pub enum GenClause {
  Call(Box<GenClause>, Box<GenClause>),
  Lambda(Token<String>, Box<GenClause>),
  Arg(Token<String>),
  Slot(ExprTicket),
  Seq(Box<GenClause>, Box<GenClause>),
  Const(Token<Vec<Token<String>>>),
  ThinAtom(Box<dyn Fn(SysId, &dyn DynSystem) -> Atom>),
  OwnedAtom(u64),
}

pub fn cnst(path: Token<Vec<Token<String>>>) -> GenClause { GenClause::Const(path) }
pub fn val<A: ThinAtom>(atom: A) -> GenClause {
  GenClause::ThinAtom(Box::new(move |id, sys| encode_atom_nodrop::<A>(id, sys.card(), &atom)))
}

pub fn obj<A: OwnedAtom>(atom: A) -> GenClause {
  GenClause::OwnedAtom(OBJ_STORE.add(Box::new(atom)))
}

pub fn seq(ops: impl IntoIterator<Item = GenClause>) -> GenClause {
  fn recur(mut ops: impl Iterator<Item = GenClause>) -> Option<GenClause> {
    let op = ops.next()?;
    Some(match recur(ops) {
      None => op,
      Some(rec) => GenClause::Seq(Box::new(op), Box::new(rec)),
    })
  }
  recur(ops.into_iter()).expect("Empty list provided to seq!")
}

pub fn slot(extk: ExprTicket) -> GenClause { GenClause::Slot(extk) }

pub fn arg(n: Token<String>) -> GenClause { GenClause::Arg(n) }

pub fn lambda(n: Token<String>, b: impl IntoIterator<Item = GenClause>) -> GenClause {
  GenClause::Lambda(n, Box::new(call(b)))
}

pub fn call(v: impl IntoIterator<Item = GenClause>) -> GenClause {
  v.into_iter()
    .reduce(|f, x| GenClause::Call(Box::new(f), Box::new(x)))
    .expect("Empty call expression")
}
