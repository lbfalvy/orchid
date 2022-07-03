#[derive(Debug, Clone)]
pub enum Loaded {
    Module(String),
    Namespace(Vec<String>),
}