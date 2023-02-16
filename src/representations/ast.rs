use mappable_rc::Mrc;
use itertools::Itertools;
use ordered_float::NotNan;
use std::{hash::Hash, intrinsics::likely};
use std::fmt::Debug;
use crate::utils::mrc_empty_slice;
use crate::{executor::{ExternFn, Atom}, utils::one_mrc_slice};

use super::Literal;

/// An S-expression with a type
#[derive(PartialEq, Eq, Hash)]
pub struct Expr(pub Clause, pub Mrc<[Clause]>);
impl Expr {
    pub fn into_clause(self) -> Clause {
        if likely(self.1.len() == 0) { self.0 }
        else { Clause::S('(', one_mrc_slice(self)) }
    }
}

impl Clone for Expr {
    fn clone(&self) -> Self {
        Self(self.0.clone(), Mrc::clone(&self.1))
    }
}

impl Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Expr(val, typ) = self;
        write!(f, "{:?}", val)?;
        for typ in typ.as_ref() {
            write!(f, ":{:?}", typ)?
        }
        Ok(())
    }
}

/// An S-expression as read from a source file
#[derive(PartialEq, Eq, Hash)]
pub enum Clause {
    /// A literal value, eg. `1`, `"hello"`
    Literal(Literal),
    /// A c-style name or an operator, eg. `+`, `i`, `foo::bar`
    Name{
        local: Option<String>,
        qualified: Mrc<[String]>
    },
    /// A parenthesized expression, eg. `(print out "hello")`, `[1, 2, 3]`, `{Some(t) => t}`
    S(char, Mrc<[Expr]>),
    /// An explicit expression associated with the leftmost, outermost [Clause::Auto], eg. `read @Int`
    Explicit(Mrc<Expr>),
    /// A function expression, eg. `\x. x + 1`
    Lambda(String, Mrc<[Expr]>, Mrc<[Expr]>),
    /// A parameterized expression with type inference, eg. `@T. T -> T`
    Auto(Option<String>, Mrc<[Expr]>, Mrc<[Expr]>),
    /// An opaque function, eg. an effectful function employing CPS.
    /// Preferably wrap these in an Orchid monad.
    ExternFn(ExternFn),
    /// An opaque non-callable value, eg. a file handle.
    /// Preferably wrap these in an Orchid structure.
    Atom(Atom),
    /// A placeholder for macros, eg. `$name`, `...$body`, `...$lhs:1` 
    Placeh{
        key: String,
        /// None => matches one token
        /// Some((prio, nonzero)) =>
        ///     prio is the sizing priority for the vectorial (higher prio grows first)
        ///     nonzero is whether the vectorial matches 1..n or 0..n tokens
        vec: Option<(usize, bool)>
    },
}
impl Clause {
    pub fn body(&self) -> Option<Mrc<[Expr]>> {
        match self {
            Self::Auto(_, _, body) | 
            Self::Lambda(_, _, body) |
            Self::S(_, body) => Some(Mrc::clone(body)),
            _ => None
        }
    }
    pub fn typ(&self) -> Option<Mrc<[Expr]>> {
        match self {
            Self::Auto(_, typ, _) | Self::Lambda(_, typ, _) => Some(Mrc::clone(typ)),
            _ => None
        }
    }
    pub fn into_expr(self) -> Expr {
        if let Self::S('(', body) = &self {
            if body.len() == 1 { body[0].clone() }
            else { Expr(self, mrc_empty_slice()) }
        } else { Expr(self, mrc_empty_slice()) }
    }
    pub fn from_exprv(exprv: Mrc<[Expr]>) -> Option<Clause> {
        if exprv.len() == 0 { None }
        else if exprv.len() == 1 { Some(exprv[0].clone().into_clause()) }
        else { Some(Self::S('(', exprv)) }
    }
}

impl Clone for Clause {
    fn clone(&self) -> Self {
        match self {
            Self::S(c, b) => Self::S(*c, Mrc::clone(b)),
            Self::Auto(n, t, b) => Self::Auto(
                n.clone(), Mrc::clone(t), Mrc::clone(b)
            ),
            Self::Name { local: l, qualified: q } => Self::Name {
                local: l.clone(), qualified: Mrc::clone(q)
            },
            Self::Lambda(n, t, b) => Self::Lambda(
                n.clone(), Mrc::clone(t), Mrc::clone(b)
            ),
            Self::Placeh{key, vec} => Self::Placeh{key: key.clone(), vec: *vec},
            Self::Literal(l) => Self::Literal(l.clone()),
            Self::ExternFn(nc) => Self::ExternFn(nc.clone()),
            Self::Atom(a) => Self::Atom(a.clone()),
            Self::Explicit(expr) => Self::Explicit(Mrc::clone(expr))
        }
    }
}

fn fmt_expr_seq(it: &mut dyn Iterator<Item = &Expr>, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for item in Itertools::intersperse(it.map(Some), None) { match item {
        Some(expr) => write!(f, "{:?}", expr),
        None => f.write_str(" "),
    }? }
    Ok(())
}

impl Debug for Clause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Literal(arg0) => write!(f, "{:?}", arg0),
            Self::Name{local, qualified} =>
                if let Some(local) = local {write!(f, "{}`{}`", qualified.join("::"), local)}
                else {write!(f, "{}", qualified.join("::"))},
            Self::S(del, items) => {
                f.write_str(&del.to_string())?;
                fmt_expr_seq(&mut items.iter(), f)?;
                f.write_str(match del {
                    '(' => ")", '[' => "]", '{' => "}",
                    _ => "CLOSING_DELIM"
                })
            },
            Self::Lambda(name, argtyp, body) => {
                f.write_str("\\")?;
                f.write_str(name)?;
                f.write_str(":")?; fmt_expr_seq(&mut argtyp.iter(), f)?; f.write_str(".")?;
                fmt_expr_seq(&mut body.iter(), f)
            },
            Self::Auto(name, argtyp, body) => {
                f.write_str("@")?;
                f.write_str(&name.clone().unwrap_or_default())?;
                f.write_str(":")?; fmt_expr_seq(&mut argtyp.iter(), f)?; f.write_str(".")?;
                fmt_expr_seq(&mut body.iter(), f)
            },
            Self::Placeh{key, vec: None} => write!(f, "${key}"),
            Self::Placeh{key, vec: Some((prio, true))} => write!(f, "...${key}:{prio}"),
            Self::Placeh{key, vec: Some((prio, false))} => write!(f, "..${key}:{prio}"),
            Self::ExternFn(nc) => write!(f, "{nc:?}"),
            Self::Atom(a) => write!(f, "{a:?}"),
            Self::Explicit(expr) => write!(f, "@{:?}", expr.as_ref())
        }
    }
}

/// A substitution rule as read from the source
#[derive(PartialEq, Eq, Hash)]
pub struct Rule {
    pub source: Mrc<[Expr]>,
    pub prio: NotNan<f64>,
    pub target: Mrc<[Expr]>
}

impl Clone for Rule {
    fn clone(&self) -> Self {
        Self {
            source: Mrc::clone(&self.source),
            prio: self.prio,
            target: Mrc::clone(&self.target)
        }
    }
}

impl Debug for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} ={}=> {:?}", self.source, self.prio, self.target)
    }
}