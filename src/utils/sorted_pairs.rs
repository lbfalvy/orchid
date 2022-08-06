use std::ops::Add;

/// Combine two sorted iterators with their mapper function into a sorted iterator of pairs
pub struct SortedPairs<L, R, IL, IR, ML, MR, O> {
    left: IL, right: IR,
    left_map: ML, right_map: MR,
    left_buf: Vec<(L, O)>, right_buf: Vec<(R, O)>
}

impl<L, R, IL, IR, ML, MR, O> SortedPairs<L, R, IL, IR, ML, MR, O>
where IL: Iterator<Item = L>, IR: Iterator<Item = R>,
    ML: Fn(L) -> O, MR: Fn(R) -> O,
    O: Ord + Add + Clone
{
    pub fn new(left: IL, right: IR, left_map: ML, right_map: MR) -> Self {
        Self {
            left, right, left_map, right_map,
            left_buf: Vec::new(),
            right_buf: Vec::new()
        }
    }
}

impl<'a, L: 'a, R: 'a, IL: 'a, IR: 'a, ML: 'a, MR: 'a, O: 'a> Iterator
for &'a mut SortedPairs<L, R, IL, IR, ML, MR, O>
where IL: Iterator<Item = L>, IR: Iterator<Item = R>,
    ML: Fn(L) -> O, MR: Fn(R) -> O,
    O: Ord + Add + Clone,
{
    type Item = (&'a L, &'a R);

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}