use mappable_rc::Mrc;

use crate::utils::collect_to_mrc;

use super::super::representations::typed::{Clause, Expr};

fn normalize(Expr(clause, typ): Expr) -> Expr {
    todo!()
}

fn collect_autos(
    Expr(clause, typ): Expr,
    arg_types: Vec<Mrc<[Clause]>>,
    indirect_argt_trees: Vec<Mrc<[Clause]>>,
    sunk_types: &mut dyn Iterator<Item = Clause>
) -> (Vec<Mrc<[Clause]>>, Expr) {
    if let Clause::Auto(argt, body) = clause {
        
    }
    else {(
        arg_types,
        Expr(
            clause,
            collect_to_mrc(
                typ.iter().cloned()
                .chain(sunk_types)
            )
        )
    )}
}