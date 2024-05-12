use orchid_api::expr::Expr;

pub trait System: Send {
  fn consts(&self) -> Expr;
}

