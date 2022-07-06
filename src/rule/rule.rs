use std::cmp::{min, max};

use hashbrown::HashSet;

use crate::expression::Expr;

pub trait Rule {
    type OutIter: Iterator<Item = Option<Expr>>;
    /// The minimum and maximum set of symbols this rule may match.
    fn len(&self) -> (Option<usize>, Option<usize>);
    /// The exact tokens the pattern consumes (None if varies)
    fn consumes(&self) -> Option<HashSet<Vec<String>>>;
    /// The exact tokens the pattern produces (None if varies)
    fn produces(&self) -> Option<HashSet<Vec<String>>>;
    /// Check if the slice matches, and produce the necessary transformations
    fn produce(&self, base: &[Expr]) -> Option<Self::OutIter>;
    /// Try all subsections of Vec of appropriate size, longest first, front-to-back
    /// Match the first, execute the substitution, return the vector and whether any
    /// substitutions happened
    fn apply(&self, mut base: Vec<Expr>) -> (Vec<Expr>, bool) {
        let len_range = self.len();
        let lo = max(len_range.0.unwrap_or(1), 1);
        let hi = min(len_range.1.unwrap_or(base.len()), base.len());
        for width in (lo..hi).rev() {
            let starts = (0..base.len() - width).into_iter();
            let first_match = starts.filter_map(|start| {
                self.produce(&base[start..start+width])
                    .map(|res| (start, res))
            }).next();
            if let Some((start, substitution)) = first_match {
                let diff = substitution.enumerate().filter_map(|(i, opt)| opt.map(|val| (i, val)));
                for (idx, item) in diff { base[start + idx] = item }
                return (base, true)
            }
        }
        (base, false)
    }
}