use std::collections::HashMap;

mod resolve_names;


#[derive(Debug, Clone)]
pub struct Project {
    pub modules: HashMap<Vec<String>, Module>,
}

#[derive(Debug, Clone)]
pub struct Export {
    isSymbol: bool,
    subpaths: HashMap<String, Export>
}

#[derive(Debug, Clone)]
pub struct Module {
    pub substitutions: Vec<Substitution>,
    pub exports: HashMap<String, Export>,
    pub all_ops: Vec<String>
}

#[derive(Debug, Clone)]
pub struct Substitution {
    pub source: Expr,
    pub priority: f64,
    pub target: Expr
}

#[derive(Debug, Clone)]
pub enum Literal {
    Num(f64),
    Int(u64),
    Char(char),
    Str(String),
}

#[derive(Debug, Clone)]
pub enum Token {
    Literal(Literal),
    Name(String),
    Bound,
    S(Vec<Expr>),
    Lambda(Vec<Vec<usize>>, Option<Box<Expr>>, Vec<Expr>),
    Auto(Option<Vec<Vec<usize>>>, Option<Box<Expr>>, Vec<Expr>)
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub token: Token,
    pub typ: Box<Expr>
}
