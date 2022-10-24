// mod hindley_milner;

#[derive(Clone, Hash, PartialEq, Eq)]
pub enum Expression<L, V, O, F> {
    Literal(L),
    Variable(V),
    Operation(O, Vec<Expression<L, V, O, F>>),
    Lazy(F)
}

pub struct Rule {

}