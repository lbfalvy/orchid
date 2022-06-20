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
    Name {
        qualified: Vec<String>,
        local: Option<String>
    },
    S(Vec<Expr>),
    Lambda(String, Option<Box<Expr>>, Vec<Expr>),
    Auto(Option<String>, Option<Box<Expr>>, Vec<Expr>)
}

#[derive(Debug, Clone)]
pub struct Expr {
    pub token: Token,
    pub typ: Option<Box<Expr>>
}