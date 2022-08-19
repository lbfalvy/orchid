use std::fmt::Debug;

use mappable_rc::Mrc;

use crate::expression::{Expr, Clause};
use crate::unwrap_or_continue;
use crate::utils::iter::box_empty;
use crate::utils::{Side, Cache, mrc_derive, mrc_try_derive, to_mrc_slice};

use super::State;
use super::split_at_max_vec::split_at_max_vec;

/// Tuple with custom cloning logic
#[derive(Debug, Eq, PartialEq, Hash)]
pub struct CacheEntry<'a>(Mrc<[Expr]>, &'a SliceMatcherDnC);
impl<'a> Clone for CacheEntry<'a> {
    fn clone(&self) -> Self {
        let CacheEntry(mrc, matcher) = self;
        CacheEntry(Mrc::clone(mrc), matcher)
    }
}


/// Matcher that applies a pattern to a slice via divide-and-conquer
/// 
/// Upon construction, it selects the clause of highest priority, then
/// initializes its internal state for matching that clause and delegates
/// the left and right halves of the pattern to two submatchers.
/// 
/// Upon matching, it uses a cache to accelerate the process of executing
/// a pattern on the entire tree.
#[derive(Clone, Eq)]
pub struct SliceMatcherDnC {
    /// The entire pattern this will match
    pattern: Mrc<[Expr]>,
    /// The exact clause this can match
    clause: Mrc<Clause>,
    /// Matcher for the parts of the pattern right from us
    right_subm: Option<Box<SliceMatcherDnC>>,
    /// Matcher for the parts of the pattern left from us
    left_subm: Option<Box<SliceMatcherDnC>>,
    /// Matcher for the body of this clause if it has one.
    /// Must be Some if pattern is (Auto, Lambda or S)
    body_subm: Option<Box<SliceMatcherDnC>>,
    /// Matcher for the type of this expression if it has one (Auto usually does)
    /// Optional
    typ_subm: Option<Box<SliceMatcherDnC>>,
}

impl PartialEq for SliceMatcherDnC {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern
    }
}

impl std::hash::Hash for SliceMatcherDnC {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.pattern.hash(state);
    }
}

impl SliceMatcherDnC {
    /// If this is true, `clause`, `typ_subm`, `body_subm` and `clause_qual_name` are meaningless.
    /// If it's false, it's also false for both side matchers.
    pub fn clause_is_vectorial(&self) -> bool {
        matches!(self.clause.as_ref(), Clause::Placeh{vec: Some(..), ..})
    }
    /// If clause is a name, the qualified name this can match
    pub fn clause_qual_name(&self) -> Option<Mrc<[String]>> {
        if let Clause::Name { qualified, .. } = self.clause.as_ref() {Some(Mrc::clone(qualified))} else {None}
    }
    /// If clause is a Placeh, the key in the state the match will be stored at
    pub fn state_key(&self) -> Option<&String> {
        if let Clause::Placeh { key, .. } = self.clause.as_ref() {Some(key)} else {None}
    }
    pub fn own_max_size(&self, total: usize) -> Option<usize> {
        if !self.clause_is_vectorial() {
            if total == self.len() {Some(total)} else {None}
        } else {
            let margin = self.min(Side::Left) + self.min(Side::Right);
            if margin + self.own_min_size() <= total {Some(total - margin)} else {None}
        }
    }
    pub fn own_min_size(&self) -> usize {
        if let Clause::Placeh { vec: Some((_, nonzero)), .. } = self.clause.as_ref() {
            if *nonzero {1} else {0}
        } else {self.len()}
    }
    
    /// Enumerate all valid subdivisions based on the reported size constraints of self and
    /// the two subranges
    pub fn valid_subdivisions(&self,
        range: Mrc<[Expr]>
    ) -> impl Iterator<Item = (Mrc<[Expr]>, Mrc<[Expr]>, Mrc<[Expr]>)> {
        let own_max = {
            if let Some(x) = self.own_max_size(range.len()) {x}
            else {return box_empty()}
        };
        let own_min = self.own_min_size();
        let lmin = self.min(Side::Left);
        let _lmax = self.max(Side::Left, range.len());
        let rmin = self.min(Side::Right);
        let _rmax = self.max(Side::Right, range.len());
        let full_len = range.len();
        Box::new((own_min..=own_max).rev().flat_map(move |own_len| {
            let wiggle = full_len - lmin - rmin - own_len;
            let range = Mrc::clone(&range);
            (0..=wiggle).map(move |offset| {
                let first_break = lmin + offset;
                let second_break = first_break + own_len;
                let left = mrc_derive(&range, |p| &p[0..first_break]);
                let mid = mrc_derive(&range, |p| &p[first_break..second_break]);
                let right = mrc_derive(&range, |p| &p[second_break..]);
                (left, mid, right)
            })
        }))
    }

    pub fn new(pattern: Mrc<[Expr]>) -> Self {
        let (clause, left_subm, right_subm) = mrc_try_derive(&pattern, |p| {
            if p.len() == 1 {Some(&p[0].0)} else {None}
        }).map(|e| (e, None, None))
        .or_else(|| split_at_max_vec(Mrc::clone(&pattern)).map(|(left, _, right)| (
            mrc_derive(&pattern, |p| &p[left.len()].0),
            if !left.is_empty() {Some(Box::new(Self::new(left)))} else {None},
            if !right.is_empty() {Some(Box::new(Self::new(right)))} else {None}
        )))
        .unwrap_or_else(|| (
            mrc_derive(&pattern, |p| &p[0].0),
            None,
            Some(Box::new(Self::new(mrc_derive(&pattern, |p| &p[1..]))))
        ));
        Self {
            pattern, right_subm, left_subm,
            clause: Mrc::clone(&clause),
            body_subm: clause.body().map(|b| Box::new(Self::new(b))),
            typ_subm: clause.typ().map(|t| Box::new(Self::new(t)))
        }
    }

    /// The shortest slice this pattern can match
    fn len(&self) -> usize {
        if self.clause_is_vectorial() {
            self.min(Side::Left) + self.min(Side::Right) + self.own_min_size()
        } else {self.pattern.len()}
    }
    /// Pick a subpattern based on the parameter
    fn side(&self, side: Side) -> Option<&SliceMatcherDnC> {
        match side {
            Side::Left => &self.left_subm,
            Side::Right => &self.right_subm
        }.as_ref().map(|b| b.as_ref())
    }
    /// The shortest slice the given side can match
    fn min(&self, side: Side) -> usize {self.side(side).map_or(0, |right| right.len())}
    /// The longest slice the given side can match
    fn max(&self, side: Side, total: usize) -> usize {
        self.side(side).map_or(0, |m| if m.clause_is_vectorial() {
            total - self.min(side.opposite()) - self.own_min_size()
        } else {m.len()})
    }
    /// Take the smallest possible slice from the given side
    fn slice_min<'a>(&self, side: Side, range: &'a [Expr]) -> &'a [Expr] {
        side.slice(self.min(side), range)
    }

    /// Matches the body on a range
    /// # Panics
    /// when called on an instance that does not have a body (not Auto, Lambda or S)
    fn match_body<'a>(&'a self,
        range: Mrc<[Expr]>, cache: &Cache<CacheEntry<'a>, Option<State>>
    ) -> Option<State> {
        self.body_subm.as_ref()
            .expect("Missing body matcher")
            .match_range_cached(range, cache)
    }
    /// Matches the type and body on respective ranges
    /// # Panics
    /// when called on an instance that does not have a body (not Auto, Lambda or S)
    fn match_parts<'a>(&'a self,
        typ_range: Mrc<[Expr]>, body_range: Mrc<[Expr]>,
        cache: &Cache<CacheEntry<'a>, Option<State>>
    ) -> Option<State> {
        let typ_state = if let Some(typ) = &self.typ_subm {
            typ.match_range_cached(typ_range, cache)?
        } else {State::new()};
        let body_state = self.match_body(body_range, cache)?;
        typ_state + body_state
    }

    /// Match the specified side-submatcher on the specified range with the cache
    /// In absence of a side-submatcher empty ranges are matched to empty state
    fn apply_side_with_cache<'a>(&'a self,
        side: Side, range: Mrc<[Expr]>,
        cache: &Cache<CacheEntry<'a>, Option<State>>
    ) -> Option<State> {
        match &self.side(side) {
            None => {
                if !range.is_empty() {None}
                else {Some(State::new())}
            },
            Some(m) => cache.try_find(&CacheEntry(range, m)).map(|s| s.as_ref().to_owned())
        }
    }

    fn match_range_scalar_cached<'a>(&'a self,
        target: Mrc<[Expr]>,
        cache: &Cache<CacheEntry<'a>, Option<State>>
    ) -> Option<State> {
        let pos = self.min(Side::Left);
        if target.len() != self.pattern.len() {return None}
        let mut own_state = (
            self.apply_side_with_cache(Side::Left, mrc_derive(&target, |t| &t[0..pos]), cache)?
            + self.apply_side_with_cache(Side::Right, mrc_derive(&target, |t| &t[pos+1..]), cache)
        )?;
        match (self.clause.as_ref(), &target.as_ref()[pos].0) {
            (Clause::Literal(val), Clause::Literal(tgt)) => {
                if val == tgt {Some(own_state)} else {None}
            }
            (Clause::Placeh{key, vec: None}, _) => {
                own_state.insert_scalar(&key, &target[pos])
            }
            (Clause::S(c, _), Clause::S(c_tgt, body_range)) => {
                if c != c_tgt {return None}
                own_state + self.match_parts(to_mrc_slice(vec![]), Mrc::clone(body_range), cache)
            }
            (Clause::Name{qualified, ..}, Clause::Name{qualified: q_tgt, ..}) => {
                if qualified == q_tgt {Some(own_state)} else {None}
            }
            (Clause::Lambda(name, _, _), Clause::Lambda(name_tgt, typ_tgt, body_tgt)) => {
                // Primarily, the name works as a placeholder
                if let Some(state_key) = name.strip_prefix('$') {
                    own_state = own_state.insert_name(state_key, name_tgt)?
                } else if name != name_tgt {return None}
                // ^ But if you're weird like that, it can also work as a constraint
                own_state + self.match_parts(Mrc::clone(typ_tgt), Mrc::clone(body_tgt), cache)
            }
            (Clause::Auto(name_opt, _, _), Clause::Auto(name_range, typ_range, body_range)) => {
                if let Some(name) = name_opt {
                    // TODO: Enforce this at construction, on a type system level
                    let state_key = name.strip_prefix('$')
                        .expect("Auto patterns may only reference, never enforce the name");
                    own_state = own_state.insert_name_opt(state_key, name_range.as_ref())?
                }
                own_state + self.match_parts(Mrc::clone(typ_range), Mrc::clone(body_range), cache)
            },
            _ => None
        }
    }

    /// Match the range with a vectorial _assuming we are a vectorial_
    fn match_range_vectorial_cached<'a>(&'a self,
        name: &str,
        target: Mrc<[Expr]>,
        cache: &Cache<CacheEntry<'a>, Option<State>>
    ) -> Option<State> {
        // Step through valid slicings based on reported size constraints in order
        // from longest own section to shortest and from left to right
        for (left, own, right) in self.valid_subdivisions(target) {
            let sides_result = unwrap_or_continue!(
                self.apply_side_with_cache(Side::Left, left, cache)
            ) + self.apply_side_with_cache(Side::Right, right, cache);
            return Some(unwrap_or_continue!(
                unwrap_or_continue!(sides_result)
                    .insert_vec(name, own.as_ref())
            ))
        }
        None
    }

    /// Try and match the specified range
    pub fn match_range_cached<'a>(&'a self,
        target: Mrc<[Expr]>,
        cache: &Cache<CacheEntry<'a>, Option<State>>
    ) -> Option<State> {
        if self.pattern.is_empty() {
            return if target.is_empty() {Some(State::new())} else {None}
        }
        match self.clause.as_ref() {
            Clause::Placeh{key, vec: Some(_)} =>
                self.match_range_vectorial_cached(key, target, cache),
            _ => self.match_range_scalar_cached(target, cache)
        }
    }

    pub fn get_matcher_cache<'a>()
    -> Cache<'a, CacheEntry<'a>, Option<State>> {
        Cache::new(
            |CacheEntry(tgt, matcher), cache| {
                matcher.match_range_cached(tgt, cache)
            }
        )
    }

    pub fn match_range(&self, target: Mrc<[Expr]>) -> Option<State> {
        self.match_range_cached(target, &Self::get_matcher_cache())
    }
}

impl Debug for SliceMatcherDnC {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Matcher")
            .field("clause", &self.clause)
            .field("vectorial", &self.clause_is_vectorial())
            .field("min", &self.len())
            .field("left", &self.left_subm)
            .field("right", &self.right_subm)
            .field("lmin", &self.min(Side::Left))
            .field("rmin", &self.min(Side::Right))
            .finish()
    }
}
