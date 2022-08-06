use hashbrown::HashMap;
use itertools::Itertools;

use crate::expression::{Expr, Clause};
use crate::unwrap_or_continue;
use crate::utils::{Side, Cache};
use super::super::RuleError;
use super::State;

fn split_at_max_vec(pattern: &[Expr]) -> Option<(&[Expr], (&str, usize), &[Expr])> {
    let rngidx = pattern.iter().position_max_by_key(|ex| {
        if let Expr(Clause::Placeh(_, Some(prio)), _) = ex { *prio as i64 } else { -1 }
    })?;
    let (left, not_left) = pattern.split_at(rngidx);
    let (placeh, right) = if rngidx == pattern.len() {
        (&not_left[0].0, [].as_slice())
    } else {
        let (placeh_unary_slice, right) = pattern.split_at(rngidx + 1);
        (&placeh_unary_slice[0].0, right)
    };
    if let Clause::Placeh(name, Some(prio)) = placeh {
        Some((left, (name, *prio), right))
    } else {None}
}

/// Matcher that applies a pattern to a slice via divide-and-conquer
/// 
/// Upon construction, it selects the clause of highest priority, then
/// initializes its internal state for matching that clause and delegates
/// the left and right halves of the pattern to two submatchers.
/// 
/// Upon matching, it uses a cache to accelerate the process of executing
/// a pattern on the entire tree.
#[derive(Debug, Clone, Eq)]
pub struct SliceMatcherDnC<'a> {
    /// The entire pattern this will match
    pattern: &'a [Expr],
    /// The exact clause this can match
    clause: &'a Clause,
    /// Matcher for the parts of the pattern right from us
    right_subm: Option<Box<SliceMatcherDnC<'a>>>,
    /// Matcher for the parts of the pattern left from us
    left_subm: Option<Box<SliceMatcherDnC<'a>>>,
    /// Matcher for the body of this clause if it has one.
    /// Must be Some if pattern is (Auto, Lambda or S)
    body_subm: Option<Box<SliceMatcherDnC<'a>>>,
    /// Matcher for the type of this expression if it has one (Auto usually does)
    /// Optional
    typ_subm: Option<Box<SliceMatcherDnC<'a>>>,
}

impl<'a> PartialEq for SliceMatcherDnC<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl<'a> std::hash::Hash for SliceMatcherDnC<'a> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pattern.hash(state);
    }
}

impl<'a> SliceMatcherDnC<'a> {
    /// If this is true, `clause`, `typ_subm`, `body_subm` and `clause_qual_name` are meaningless.
    /// If it's false, it's also false for both side matchers.
    pub fn clause_is_vectorial(&self) -> bool {
        if let Clause::Placeh(_, Some(_)) = self.clause {true} else {false}
    }
    /// If clause is a name, the qualified name this can match
    pub fn clause_qual_name(&self) -> Option<&'a Vec<String>> {
        if let Clause::Name { qualified, .. } = self.clause {Some(qualified)} else {None}
    }
    /// If clause is a Placeh, the key in the state the match will be stored at
    pub fn state_key(&self) -> Option<&'a String> {
        if let Clause::Placeh(key, _) = self.clause {Some(key)} else {None}
    }
    pub fn own_max_size(&self, total: usize) -> usize {
        if !self.clause_is_vectorial() {return self.len()}
        return total - self.min(Side::Left) - self.min(Side::Right)
    }
    /// Enumerate all valid subdivisions based on the reported size constraints of self and
    /// the two subranges
    pub fn valid_subdivisions<'b>(&self,
        range: &'b [Expr]
    ) -> impl Iterator<Item = (&'b [Expr], &'b [Expr], &'b [Expr])> {
        let own_size = self.own_max_size(range.len());
        let lmin = self.min(Side::Left);
        let lmax = self.max(Side::Left, range.len());
        let rmin = self.min(Side::Right);
        let rmax = self.max(Side::Right, range.len());
        let full_len = range.len();
        (1..=own_size).rev().flat_map(move |own_len| {
            let wiggle = full_len - lmin - rmin - own_len;
            (0..wiggle).map(move |offset| {
                let first_break = lmin + offset;
                let (left, rest) = range.split_at(first_break);
                let (mid, right) = rest.split_at(own_len);
                (left, mid, right)
            })
        })
    }

    pub fn new(pattern: &'a [Expr]) -> Self {
        let (Expr(clause, _), left_subm, right_subm) = if pattern.len() == 1 {
            (&pattern[0], None, None)
        } else if let Some((left, _, right)) = split_at_max_vec(pattern) {(
            &pattern[left.len()],
            Some(Box::new(Self::new(left))),
            Some(Box::new(Self::new(right)))
        )} else {(
            &pattern[0],
            None, 
            Some(Box::new(Self::new(&pattern[1..])))
        )};
        Self {
            pattern, right_subm, left_subm, clause,
            body_subm: clause.body().map(|b| Box::new(Self::new(b))),
            typ_subm: clause.typ().map(|t| Box::new(Self::new(t)))
        }
    }

    /// The shortest slice this pattern can match
    fn len(&self) -> usize {self.pattern.len()}
    /// Pick a subpattern based on the parameter
    fn side(&self, side: Side) -> Option<&Box<SliceMatcherDnC<'a>>> {
        match side {
            Side::Left => &self.left_subm,
            Side::Right => &self.right_subm
        }.as_ref()
    }
    /// The shortest slice the given side can match
    fn min(&self, side: Side) -> usize {self.side(side).map_or(0, |right| right.len())}
    /// The longest slice the given side can match
    fn max(&self, side: Side, total: usize) -> usize {
        self.side(side).map_or(0, |m| if m.clause_is_vectorial() {
            total - self.min(side.opposite()) - 1
        } else {m.len()})
    }
    /// Take the smallest possible slice from the given side
    fn slice_min<'b>(&self, side: Side, range: &'b [Expr]) -> &'b [Expr] {
        side.slice(self.min(side), range)
    }

    /// Matches the body on a range
    /// # Panics
    /// when called on an instance that does not have a body (not Auto, Lambda or S)
    fn match_body<'b>(&'a self,
        range: &'b [Expr], cache: &Cache<(&'b [Expr], &'a SliceMatcherDnC<'a>), Option<State>>
    ) -> Option<State> {
        self.body_subm.as_ref().unwrap().match_range_cached(range, cache)
    }
    /// Matches the type and body on respective ranges
    /// # Panics
    /// when called on an instance that does not have a body (not Auto, Lambda or S)
    fn match_parts<'b>(&'a self,
        typ_range: &'b [Expr], body_range: &'b [Expr],
        cache: &Cache<(&'b [Expr], &'a SliceMatcherDnC<'a>), Option<State>>
    ) -> Option<State> {
        let typ_state = if let Some(typ) = &self.typ_subm {
            typ.match_range_cached(&typ_range, cache)?
        } else {State::new()};
        let body_state = self.match_body(body_range, cache)?;
        typ_state + body_state
    }

    /// Match the specified side-submatcher on the specified range with the cache
    /// In absence of a side-submatcher empty ranges are matched to empty state
    fn apply_side_with_cache<'b>(&'a self,
        side: Side, range: &'b [Expr],
        cache: &Cache<(&'b [Expr], &'a SliceMatcherDnC<'a>), Option<State>>
    ) -> Option<State> {
        match &self.side(side) {
            None => {
                if range.len() != 0 {None}
                else {Some(State::new())}
            },
            Some(m) => cache.try_find(&(range, &m)).map(|s| s.as_ref().to_owned())
        }
    }

    fn match_range_scalar_cached<'b>(&'a self,
        target: &'b [Expr],
        cache: &Cache<(&'b [Expr], &'a SliceMatcherDnC<'a>), Option<State>>
    ) -> Option<State> {
        let pos = self.min(Side::Left);
        if target.len() != self.pattern.len() {return None}
        let mut own_state = (
            self.apply_side_with_cache(Side::Left, &target[0..pos], cache)?
            + self.apply_side_with_cache(Side::Right, &target[pos+1..], cache)
        )?;
        match (self.clause, &target[pos].0) {
            (Clause::Literal(val), Clause::Literal(tgt)) => {
                if val == tgt {Some(own_state)} else {None}
            }
            (Clause::Placeh(name, None), _) => {
                own_state.insert(name, &[target[pos].clone()])
            }
            (Clause::S(c, _), Clause::S(c_tgt, body_range)) => {
                if c != c_tgt {return None}
                own_state + self.match_parts(&[], body_range, cache)
            }
            (Clause::Name{qualified, ..}, Clause::Name{qualified: q_tgt, ..}) => {
                if qualified == q_tgt {Some(own_state)} else {None}
            }
            (Clause::Lambda(name, _, _), Clause::Lambda(name_tgt, typ_tgt, body_tgt)) => {
                // Primarily, the name works as a placeholder
                if let Some(state_key) = name.strip_prefix("$") {
                    own_state = own_state.insert(
                        state_key,
                        &[Expr(Clause::Name{
                            local: Some(name_tgt.clone()),
                            qualified: vec![name_tgt.clone()]
                        }, None)]
                    )?
                    // But if you're weird like that, it can also work as a constraint
                } else if name != name_tgt {return None}
                own_state + self.match_parts(typ_tgt, body_tgt, cache)
            }
            (Clause::Auto(name_opt, _, _), Clause::Auto(name_range, typ_range, body_range)) => {
                if let Some(name) = name_opt {
                    if let Some(state_name) = name.strip_prefix("$") {
                        own_state = own_state.insert(
                            state_name,
                            &[Expr(Clause::Name{
                                local: name_range.clone(),
                                qualified: name_range.as_ref()
                                    .map(|s| vec![s.clone()])
                                    .unwrap_or_default()
                            }, None)]
                        )?
                        // TODO: Enforce this at construction, on a type system level
                    } else {panic!("Auto patterns may only reference, never enforce the name")}
                }
                own_state + self.match_parts(typ_range, body_range, cache)
            },
            _ => None
        }
    }

    /// Match the range with a vectorial _assuming we are a vectorial_
    fn match_range_vectorial_cached<'b>(&'a self,
        name: &str,
        target: &'b [Expr],
        cache: &Cache<(&'b [Expr], &'a SliceMatcherDnC<'a>), Option<State>>
    ) -> Option<State> {
        // Step through valid slicings based on reported size constraints in order
        // from longest own section to shortest and from left to right
        for (left, own, right) in self.valid_subdivisions(target) {
            let left_result = unwrap_or_continue!(self.apply_side_with_cache(Side::Left, left, cache));
            let right_result = unwrap_or_continue!(self.apply_side_with_cache(Side::Right, right, cache));
            return Some(unwrap_or_continue!(
                right_result.clone()
                + left_result.insert(name, own)
            ))
        }
        return None
    }

    /// Try and match the specified range
    pub fn match_range_cached<'b>(&'a self,
        target: &'b [Expr],
        cache: &Cache<(&'b [Expr], &'a SliceMatcherDnC<'a>), Option<State>>
    ) -> Option<State> {
        if self.pattern.len() == 0 {
            return if target.len() == 0 {Some(State::new())} else {None}
        }
        match self.clause {
            Clause::Placeh(name, Some(_)) => self.match_range_vectorial_cached(name, target, cache),
            _ => self.match_range_scalar_cached(target, cache)
        }
    }

    pub fn match_range(&self, target: &[Expr]) -> Option<State> {
        self.match_range_cached(target,&Cache::<(&[Expr], &SliceMatcherDnC), _>::new(
            |(tgt, matcher), cache| {
                matcher.match_range_cached(tgt, cache)
            }
        ))
    }
}

pub fn verify_scalar_vec(pattern: &Expr, is_vec: &mut HashMap<String, bool>)
-> Result<(), String> {
    let verify_clause = |clause: &Clause, is_vec: &mut HashMap<String, bool>| -> Result<(), String> {
        match clause {
            Clause::Placeh(name, prio) => {
                if let Some(known) = is_vec.get(name) {
                    if known != &prio.is_some() { return Err(name.to_string()) }
                } else {
                    is_vec.insert(name.clone(), prio.is_some());
                }
            }
            Clause::Auto(name, typ, body) => {
                if let Some(key) = name.as_ref().map(|key| key.strip_prefix("$")).flatten() {
                    if is_vec.get(key) == Some(&true) { return Err(key.to_string()) }
                }
                typ.iter().try_for_each(|e| verify_scalar_vec(e, is_vec))?;
                body.iter().try_for_each(|e| verify_scalar_vec(e, is_vec))?;
            }
            Clause::Lambda(name, typ, body) => {
                if let Some(key) = name.strip_prefix("$") {
                    if is_vec.get(key) == Some(&true) { return Err(key.to_string()) }
                }
                typ.iter().try_for_each(|e| verify_scalar_vec(e, is_vec))?;
                body.iter().try_for_each(|e| verify_scalar_vec(e, is_vec))?;
            }
            Clause::S(_, body) => {
                body.iter().try_for_each(|e| verify_scalar_vec(e, is_vec))?;
            }
            _ => ()
        };
        Ok(())
    };
    let Expr(val, typ_opt) = pattern;
    verify_clause(val, is_vec)?;
    if let Some(typ) = typ_opt {
        verify_scalar_vec(typ, is_vec)?;
    }
    return Ok(())
}

pub fn execute(mut src: Vec<Expr>, mut tgt: Vec<Expr>, mut input: Vec<Expr>)
-> Result<(Vec<Expr>, bool), RuleError> {
    // Static values
    let prefix_expr = Expr(Clause::Placeh("::prefix".to_string(), Some(0)), None);
    let postfix_expr = Expr(Clause::Placeh("::postfix".to_string(), Some(0)), None);
    // Dimension check
    let mut is_vec_db = HashMap::new();
    src.iter().try_for_each(|e| verify_scalar_vec(e, &mut is_vec_db))
        .map_err(RuleError::ScalarVecMismatch)?;
    tgt.iter().try_for_each(|e| verify_scalar_vec(e, &mut is_vec_db))
        .map_err(RuleError::ScalarVecMismatch)?;
    // Prefix or postfix to match the full vector
    let head_multi = if let Clause::Placeh(_, Some(_)) = src.first().unwrap().0 {true} else {false};
    let tail_multi = if let Clause::Placeh(_, Some(_)) = src.last().unwrap().0 {true} else {false};
    if !head_multi {
        src.insert(0, prefix_expr.clone());
        tgt.insert(0, prefix_expr.clone());
    }
    if !tail_multi {
        src.push(postfix_expr.clone());
        tgt.push(postfix_expr.clone());
    }
    todo!()
}