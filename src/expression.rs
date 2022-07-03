use ordered_float::NotNan;
use std::{fmt::Debug};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Literal {
    Num(NotNan<f64>),
    Int(u64),
    Char(char),
    Str(String),
}

/// An S-expression with a type
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Expr(pub Clause, pub Option<Box<Expr>>);

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
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Clause {
    Literal(Literal),
    Name(Vec<String>),
    S(char, Vec<Expr>),
    Lambda(String, Vec<Expr>, Vec<Expr>),
    Auto(Option<String>, Vec<Expr>, Vec<Expr>),
    Parameter(String)
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