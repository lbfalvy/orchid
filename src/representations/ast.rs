use mappable_rc::Mrc;
use itertools::Itertools;
use ordered_float::NotNan;
use std::hash::Hash;
use std::fmt::Debug;
use crate::executor::{ExternFn, Atom};

use super::Literal;

/// An S-expression with a type
#[derive(PartialEq, Eq, Hash)]
pub struct Expr(pub Clause, pub Mrc<[Clause]>);

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
    Literal(Literal),
    Name{
        local: Option<String>,
        qualified: Mrc<[String]>
    },
    S(char, Mrc<[Expr]>),
    Lambda(String, Mrc<[Expr]>, Mrc<[Expr]>),
    Auto(Option<String>, Mrc<[Expr]>, Mrc<[Expr]>),
    ExternFn(ExternFn),
    Atom(Atom),
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
            Clause::Auto(_, _, body) | 
            Clause::Lambda(_, _, body) |
            Clause::S(_, body) => Some(Mrc::clone(body)),
            _ => None
        }
    }
    pub fn typ(&self) -> Option<Mrc<[Expr]>> {
        match self {
            Clause::Auto(_, typ, _) | Clause::Lambda(_, typ, _) => Some(Mrc::clone(typ)),
            _ => None
        }
    }
}

impl Clone for Clause {
    fn clone(&self) -> Self {
        match self {
            Clause::S(c, b) => Clause::S(*c, Mrc::clone(b)),
            Clause::Auto(n, t, b) => Clause::Auto(
                n.clone(), Mrc::clone(t), Mrc::clone(b)
            ),
            Clause::Name { local: l, qualified: q } => Clause::Name {
                local: l.clone(), qualified: Mrc::clone(q)
            },
            Clause::Lambda(n, t, b) => Clause::Lambda(
                n.clone(), Mrc::clone(t), Mrc::clone(b)
            ),
            Clause::Placeh{key, vec} => Clause::Placeh{key: key.clone(), vec: *vec},
            Clause::Literal(l) => Clause::Literal(l.clone()),
            Clause::ExternFn(nc) => Clause::ExternFn(nc.clone()),
            Clause::Atom(a) => Clause::Atom(a.clone())
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
            Self::Atom(a) => write!(f, "{a:?}")
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