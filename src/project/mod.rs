use std::collections::HashMap;

mod resolve_names;
mod prefix;
mod name_resolver;
mod expr;

#[derive(Debug, Clone)]
pub struct Project {
    pub modules: HashMap<Vec<String>, Module>,
}

#[derive(Debug, Clone)]
pub struct Module {
    pub substitutions: Vec<Substitution>,
    pub exports: Vec<String>,
    pub references: Vec<Vec<String>>
}

#[derive(Debug, Clone)]
pub struct Substitution {
    pub source: expr::Expr,
    pub priority: f64,
    pub target: expr::Expr
}
