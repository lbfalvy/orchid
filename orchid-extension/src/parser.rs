use futures::future::LocalBoxFuture;
use orchid_base::error::OrcRes;
use orchid_base::name::Sym;
use orchid_base::parse::{Comment, Snippet};

use crate::expr::Expr;
use crate::gen_expr::GExpr;
use crate::system::SysCtx;
use crate::tree::GenTokTree;

pub type GenSnippet<'a> = Snippet<'a, Expr, GExpr>;

pub trait Parser: Send + Sync + Sized + Default + 'static {
	const LINE_HEAD: &'static str;
	fn parse(
		ctx: SysCtx,
		module: Sym,
		exported: bool,
		comments: Vec<Comment>,
		line: GenSnippet<'_>,
	) -> impl Future<Output = OrcRes<Vec<GenTokTree>>> + '_;
}

pub trait DynParser: Send + Sync + 'static {
	fn line_head(&self) -> &'static str;
	fn parse<'a>(
		&self,
		ctx: SysCtx,
		module: Sym,
		exported: bool,
		comments: Vec<Comment>,
		line: GenSnippet<'a>,
	) -> LocalBoxFuture<'a, OrcRes<Vec<GenTokTree>>>;
}

impl<T: Parser> DynParser for T {
	fn line_head(&self) -> &'static str { Self::LINE_HEAD }
	fn parse<'a>(
		&self,
		ctx: SysCtx,
		module: Sym,
		exported: bool,
		comments: Vec<Comment>,
		line: GenSnippet<'a>,
	) -> LocalBoxFuture<'a, OrcRes<Vec<GenTokTree>>> {
		Box::pin(async move { Self::parse(ctx, module, exported, comments, line).await })
	}
}

pub type ParserObj = &'static dyn DynParser;
