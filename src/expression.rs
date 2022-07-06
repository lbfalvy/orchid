use itertools::Itertools;
use ordered_float::NotNan;
use std::{fmt::Debug};

/// An exact value
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Literal {
    Num(NotNan<f64>),
    Int(u64),
    Char(char),
    Str(String),
}

impl Debug for Literal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Num(arg0) => write!(f, "{:?}", arg0),
            Self::Int(arg0) => write!(f, "{:?}", arg0),
            Self::Char(arg0) => write!(f, "{:?}", arg0),
            Self::Str(arg0) => write!(f, "{:?}", arg0),
        }
    }
}

/// An S-expression with a type
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Expr(pub Clause, pub Option<Box<Expr>>);

impl Debug for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // f.debug_tuple("Expr").field(&self.0).field(&self.1).finish()
        let Expr(val, typ) = self;
        write!(f, "{:?}", val)?;
        if let Some(typ) = typ { write!(f, "{:?}", typ) }
        else { Ok(()) }
    }
}

impl Expr {
    /// Replace all occurences of a name in the tree with a parameter, to bypass name resolution
    pub fn bind_parameter(&mut self, name: &str) {
        self.0.bind_parameter(name);
        if let Some(typ) = &mut self.1 {
            typ.bind_parameter(name);
        }
    }
}

/// An S-expression as read from a source file
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum Clause {
    Literal(Literal),
    Name(Vec<String>),
    S(char, Vec<Expr>),
    Lambda(String, Vec<Expr>, Vec<Expr>),
    Auto(Option<String>, Vec<Expr>, Vec<Expr>),
    Parameter(String)
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
            Self::Name(arg0) => write!(f, "{}", arg0.join("::")),
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
            Self::Parameter(name) => write!(f, "`{}", name)
        }
    }
}

impl Clause {
    /// Replace all occurences of a name in the tree with a parameter, to bypass name resolution
    pub fn bind_parameter(&mut self, name: &str) {
        match self {
            Clause::Name(n) => if n.len() == 1 && n[0] == name {
                *self = Clause::Parameter(name.to_string())
            }
            Clause::S(_, exprv) => for expr in exprv { expr.bind_parameter(name) }
            Clause::Lambda(_, typ, body) | Clause::Auto(_, typ, body) => {
                for expr in typ { expr.bind_parameter(name) }
                for expr in body { expr.bind_parameter(name) }
            }
            _ => ()
        }
    }
}

/// A substitution rule as read from the source
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Rule {
    pub source: Vec<Expr>,
    pub prio: NotNan<f64>,
    pub target: Vec<Expr>
}

impl Debug for Rule {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} ={}=> {:?}", self.source, self.prio, self.target)
    }
}