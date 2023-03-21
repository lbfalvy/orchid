use std::collections::{VecDeque, HashSet};
use std::iter;
use std::hash::Hash;

use crate::unwrap_or;
use crate::utils::BoxedIter;

// TODO: move to own crate

/// Two-stage breadth-first search;
/// Instead of enumerating neighbors before returning a node, it puts visited but not yet
/// enumerated nodes in a separate queue and only enumerates them to refill the queue of children
/// one by one once it's empty. This method is preferable for generated graphs because it doesn't
/// allocate memory for the children until necessary, but it's also probably a bit slower since
/// it involves additional processing.
/// 
/// # Performance
/// `T` is cloned twice for each returned value. 
pub fn bfs<T, F, I>(init: T, neighbors: F)
-> impl Iterator<Item = T>
where T: Eq + Hash + Clone + std::fmt::Debug,
  F: Fn(T) -> I, I: Iterator<Item = T>
{
  let mut visited: HashSet<T> = HashSet::new();
  let mut visit_queue: VecDeque<T> = VecDeque::from([init]);
  let mut unpack_queue: VecDeque<T> = VecDeque::new();
  iter::from_fn(move || {
    let next = {loop {
      let next = unwrap_or!(visit_queue.pop_front(); break None);
      if !visited.contains(&next) { break Some(next) }
    }}.or_else(|| loop {
      let unpacked = unwrap_or!(unpack_queue.pop_front(); break None);
      let mut nbv = neighbors(unpacked).filter(|t| !visited.contains(t));
      if let Some(next) = nbv.next() {
        visit_queue.extend(nbv);
        break Some(next)
      }
    })?;
    visited.insert(next.clone());
    unpack_queue.push_back(next.clone());
    Some(next)
  })
}

/// Same as [bfs] but with a recursion depth limit
/// 
/// The main intent is to effectively walk infinite graphs of unknown breadth without making the
/// recursion depth dependent on the number of nodes. If predictable runtime is more important
/// than predictable depth, [bfs] with [std::iter::Iterator::take] should be used instead
pub fn bfs_upto<'a, T: 'a, F: 'a, I: 'a>(init: T, neighbors: F, limit: usize)
-> impl Iterator<Item = T> + 'a
where T: Eq + Hash + Clone + std::fmt::Debug,
  F: Fn(T) -> I, I: Iterator<Item = T>
{
  /// Newtype to store the recursion depth but exclude it from equality comparisons
  /// Because BFS visits nodes in increasing distance order, when a node is visited for the
  /// second time it will never override the earlier version of itself. This is not the case
  /// with Djikstra's algorithm, which can be conceptualised as a "weighted BFS".
  #[derive(Eq, Clone, Debug)]
  struct Wrap<U>(usize, U);
  impl<U: PartialEq> PartialEq for Wrap<U> {
    fn eq(&self, other: &Self) -> bool { self.1.eq(&other.1) }
  }
  impl<U: Hash> Hash for Wrap<U> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) { self.1.hash(state) }
  }
  bfs(Wrap(0, init), move |Wrap(dist, t)| -> BoxedIter<Wrap<T>> { // boxed because we branch
    if dist == limit {Box::new(iter::empty())}
    else {Box::new(neighbors(t).map(move |t| Wrap(dist + 1, t)))}
  }).map(|Wrap(_, t)| t)
}

#[cfg(test)]
mod tests {
  use itertools::Itertools;

  use super::*;

  type Graph = Vec<Vec<usize>>;
  fn neighbors(graph: &Graph, pt: usize) -> impl Iterator<Item = usize> + '_ {
    graph[pt].iter().copied()
  }
  fn from_neighborhood_matrix(matrix: Vec<Vec<usize>>) -> Graph {
    matrix.into_iter().map(|v| {
      v.into_iter().enumerate().filter_map(|(i, ent)| {
        if ent > 1 {panic!("Neighborhood matrices must contain binary values")}
        else if ent == 1 {Some(i)}
        else {None}
      }).collect()
    }).collect()
  }

  #[test]
  fn test_square() {
    let simple_graph = from_neighborhood_matrix(vec![
      vec![0,1,0,1,1,0,0,0],
      vec![1,0,1,0,0,1,0,0],
      vec![0,1,0,1,0,0,1,0],
      vec![1,0,1,0,0,0,0,1],
      vec![1,0,0,0,0,1,0,1],
      vec![0,1,0,0,1,0,1,0],
      vec![0,0,1,0,0,1,0,1],
      vec![0,0,0,1,1,0,1,0],
    ]);
    let scan = bfs(0, |n| neighbors(&simple_graph, n)).collect_vec();
    assert_eq!(scan, vec![0, 1, 3, 4, 2, 5, 7, 6])
  }
  #[test]
  fn test_stringbuilder() {
    let scan = bfs("".to_string(), |s| {
        vec![s.clone()+";", s.clone()+"a", s+"aaa"].into_iter()
    }).take(30).collect_vec();
    println!("{scan:?}")
  }
}