use super::{ast, ast_to_postmacro, interpreted, postmacro_to_interpreted};
use crate::Sym;

#[allow(unused)]
pub type AstError = ast_to_postmacro::Error;

/// Attempt to convert the AST processed by macros into an executable format
#[allow(unused)]
pub fn ast_to_interpreted(
  ast: &ast::Expr<Sym>,
) -> Result<interpreted::ExprInst, AstError> {
  let pmtree = ast_to_postmacro::expr(ast)?;
  Ok(postmacro_to_interpreted::expr(&pmtree))
}
