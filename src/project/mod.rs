mod rule_collector;
// pub use rule_collector::rule_collector;
mod prefix;
mod name_resolver;
mod loaded;
pub use loaded::Loaded;
mod parse_error;
mod file_loader;
pub use file_loader::file_loader;

#[derive(Debug, Clone)]
pub struct Module {
    pub rules: Vec<Rule>,
    pub exports: Vec<String>,
    pub references: Vec<Vec<String>>
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub source: super::Expr,
    pub priority: f64,
    pub target: super::Expr
}
