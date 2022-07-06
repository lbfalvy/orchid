use crate::expression::Expr;

use super::{Rule, BadState};

pub fn execute<Src, Tgt>(src: &Src, tgt: &Tgt, mut input: Vec<Expr>)
-> Result<(Vec<Expr>, bool), BadState> where Src: Rule, Tgt: Rule {
    let (range, state) = match src.scan_slice(&input) {
        Some(res) => res,
        None => return Ok((input, false))
    };
    let output = tgt.write(&state)?;
    input.splice(range, output);
    Ok((input, true))
}