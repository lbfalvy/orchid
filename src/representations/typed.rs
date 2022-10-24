use mappable_rc::Mrc;
use crate::executor::Atom;
use crate::utils::{to_mrc_slice, one_mrc_slice};
use crate::{executor::ExternFn, utils::string_from_charset};

use super::{Literal, ast_to_typed};
use super::ast;

use std::fmt::{Debug, Write};

/// Indicates whether either side needs to be wrapped. Syntax whose end is ambiguous on that side
/// must use parentheses, or forward the flag
#[derive(PartialEq, Eq)]
struct Wrap(bool, bool);

#[derive(PartialEq, Eq, Hash)]
pub struct Expr(pub Clause, pub Mrc<[Clause]>);
impl Expr {
    fn deep_fmt(&self, f: &mut std::fmt::Formatter<'_>, depth: usize, tr: Wrap) -> std::fmt::Result {
        let Expr(val, typ) = self;
        if typ.len() > 0 {
            val.deep_fmt(f, depth, Wrap(true, true))?;
            for typ in typ.as_ref() {
                f.write_char(':')?;
                typ.deep_fmt(f, depth, Wrap(true, true))?;
            }
        } else {
            val.deep_fmt(f, depth, tr)?;
        }
        Ok(())
    }
}

impl Clone for Expr {
    fn clone(&self) -> Self {
        Self(self.0.clone(), Mrc::clone(&self.1))
    }
}

impl Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.deep_fmt(f, 0, Wrap(false, false))
    }
}

#[derive(PartialEq, Eq, Hash)]
pub enum Clause {
    Literal(Literal),
    Apply(Mrc<Expr>, Mrc<Expr>),
    /// Explicit specification of an Auto value
    Explicit(Mrc<Expr>, Mrc<Expr>),
    Lambda(Option<Mrc<Clause>>, Mrc<Expr>),
    Auto(Option<Mrc<Clause>>, Mrc<Expr>),
    Argument(usize),
    ExternFn(ExternFn),
    Atom(Atom)
}

const ARGNAME_CHARSET: &str = "abcdefghijklmnopqrstuvwxyz";

fn parametric_fmt(
    f: &mut std::fmt::Formatter<'_>,
    prefix: &str, argtyp: Option<Mrc<Clause>>, body: Mrc<Expr>, depth: usize, wrap_right: bool
) -> std::fmt::Result {
    if wrap_right { f.write_char('(')?; }
    f.write_str(prefix)?;
    f.write_str(&string_from_charset(depth, ARGNAME_CHARSET))?;
    if let Some(typ) = argtyp {
        f.write_str(":")?;
        typ.deep_fmt(f, depth, Wrap(false, false))?;
    }
    f.write_str(".")?;
    body.deep_fmt(f, depth + 1, Wrap(false, false))?;
    if wrap_right { f.write_char(')')?; }
    Ok(())
}

impl Clause {
    fn deep_fmt(&self, f: &mut std::fmt::Formatter<'_>, depth: usize, Wrap(wl, wr): Wrap)
    -> std::fmt::Result {
        match self {
            Self::Literal(arg0) => write!(f, "{arg0:?}"),
            Self::ExternFn(nc) => write!(f, "{nc:?}"),
            Self::Atom(a) => write!(f, "{a:?}"),
            Self::Lambda(argtyp, body) => parametric_fmt(f,
                "\\", argtyp.as_ref().map(Mrc::clone), Mrc::clone(body), depth, wr
            ),
            Self::Auto(argtyp, body) => parametric_fmt(f,
                "@", argtyp.as_ref().map(Mrc::clone), Mrc::clone(body), depth, wr
            ),
            Self::Argument(up) => f.write_str(&string_from_charset(depth - up - 1, ARGNAME_CHARSET)),
            Self::Explicit(expr, param) => {
                if wl { f.write_char('(')?; }
                expr.deep_fmt(f, depth, Wrap(false, true))?;
                f.write_str(" @")?;
                param.deep_fmt(f, depth, Wrap(true, wr && !wl))?;
                if wl { f.write_char(')')?; }
                Ok(())
            }
            Self::Apply(func, x) => {
                if wl { f.write_char('(')?; }
                func.deep_fmt(f, depth, Wrap(false, true) )?;
                f.write_char(' ')?;
                x.deep_fmt(f, depth, Wrap(true, wr && !wl) )?;
                if wl { f.write_char(')')?; }
                Ok(())
            }
        }
    }
    pub fn wrap(self) -> Mrc<Expr> { Mrc::new(Expr(self, to_mrc_slice(vec![]))) }
    pub fn wrap_t(self, t: Clause) -> Mrc<Expr> { Mrc::new(Expr(self, one_mrc_slice(t))) }
}

impl Clone for Clause {
    fn clone(&self) -> Self {
        match self {
            Clause::Auto(t, b) => Clause::Auto(t.as_ref().map(Mrc::clone), Mrc::clone(b)),
            Clause::Lambda(t, b) => Clause::Lambda(t.as_ref().map(Mrc::clone), Mrc::clone(b)),
            Clause::Literal(l) => Clause::Literal(l.clone()),
            Clause::ExternFn(nc) => Clause::ExternFn(nc.clone()),
            Clause::Atom(a) => Clause::Atom(a.clone()),
            Clause::Apply(f, x) => Clause::Apply(Mrc::clone(f), Mrc::clone(x)),
            Clause::Explicit(f, x) => Clause::Explicit(Mrc::clone(f), Mrc::clone(x)),
            Clause::Argument(lvl) => Clause::Argument(*lvl)
        }
    }
}

impl Debug for Clause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.deep_fmt(f, 0, Wrap(false, false))
    }
}

impl TryFrom<&ast::Expr> for Expr {
    type Error = ast_to_typed::Error;
    fn try_from(value: &ast::Expr) -> Result<Self, Self::Error> {
        ast_to_typed::expr(value)
    }
}

impl TryFrom<&ast::Clause> for Clause {
    type Error = ast_to_typed::Error;
    fn try_from(value: &ast::Clause) -> Result<Self, Self::Error> {
        ast_to_typed::clause(value)
    }
}