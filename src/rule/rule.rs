use std::{cmp::{min, max}, error::Error, fmt, ops::Range};

use hashbrown::HashMap;

use crate::expression::Expr;

use super::BadState;

type State = HashMap<String, Expr>;

pub trait Rule {
    type Out: Iterator<Item = Expr>;
    /// The minimum and maximum set of symbols this rule may match.
    fn len(&self) -> (Option<usize>, Option<usize>);
    /// Check if the slice matches, and extract data
    fn read(&self, input: &[Expr]) -> Option<State>;
    /// Construct item from state
    fn write(&self, state: &State) -> Result<Self::Out, BadState>;
    /// Placeholders present in this pattern (all consumed must also be provided)
    fn placeholders(&'_ self) -> &'_ [&'_ str];
    /// Try all subsections of Vec of appropriate size, longest first, front-to-back
    /// Match the first, return the position and produced state
    fn scan_slice(&self, input: &[Expr]) -> Option<(Range<usize>, State)> {
        let len_range = self.len();
        let lo = max(len_range.0.unwrap_or(1), 1);
        let hi = min(len_range.1.unwrap_or(input.len()), input.len());
        for width in (lo..hi).rev() {
            let starts = (0..input.len() - width).into_iter();
            let first_match = starts.filter_map(|start| {
                let res = self.read(&input[start..start+width])?;
                Some((start..start+width, res))
            }).next();
            if first_match.is_some() {
                return first_match;
            }
        }
        None
    }
}

pub fn verify<Src, Tgt>(src: &Src, tgt: &Tgt) -> Option<Vec<String>> where Src: Rule, Tgt: Rule {
    let mut amiss: Vec<String> = Vec::new();
    for ent in tgt.placeholders() {
        if src.placeholders().iter().find(|x| x == &ent).is_none() {
            amiss.push(ent.to_string())
        }
    }
    if amiss.len() > 0 { Some(amiss) }
    else { None }
}