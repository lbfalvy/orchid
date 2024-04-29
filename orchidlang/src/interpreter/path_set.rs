use std::collections::VecDeque;
use std::fmt;

use hashbrown::HashMap;
use itertools::Itertools;

use crate::utils::join::join_maps;

/// A step into a [super::nort::Clause::Apply]. If [None], it steps to the
/// function. If [Some(n)], it steps to the `n`th _last_ argument.
pub type Step = Option<usize>;
fn print_step(step: Step) -> String {
  if let Some(n) = step { format!("{n}") } else { "f".to_string() }
}

/// A branching path selecting some placeholders (but at least one) in a Lambda
/// expression
#[derive(Clone)]
pub struct PathSet {
  /// The single steps through [super::nort::Clause::Apply]
  pub steps: VecDeque<Step>,
  /// if Some, it splits at a [super::nort::Clause::Apply]. If None, it ends in
  /// a [super::nort::Clause::LambdaArg]
  pub next: Option<HashMap<Step, PathSet>>,
}

impl PathSet {
  /// Create a path set for more than one target
  pub fn branch(
    steps: impl IntoIterator<Item = Step>,
    conts: impl IntoIterator<Item = (Step, Self)>,
  ) -> Self {
    let conts = conts.into_iter().collect::<HashMap<_, _>>();
    assert!(1 < conts.len(), "Branching pathsets need multiple continuations");
    Self { steps: steps.into_iter().collect(), next: Some(conts) }
  }

  /// Create a path set for one target
  pub fn end(steps: impl IntoIterator<Item = Step>) -> Self {
    Self { steps: steps.into_iter().collect(), next: None }
  }

  /// Create a path set that points to a slot that is a direct
  /// child of the given lambda with no applications. In essence, this means
  /// that this argument will be picked as the value of the expression after an
  /// arbitrary amount of subsequent discarded parameters.
  pub fn pick() -> Self { Self { steps: VecDeque::new(), next: None } }

  /// Merge two paths into one path that points to all targets of both. Only
  /// works if both paths select leaf nodes of the same partial tree.
  ///
  /// # Panics
  ///
  /// if either path selects a node the other path dissects
  pub fn overlay(self, other: Self) -> Self {
    let (mut short, mut long) = match self.steps.len() < other.steps.len() {
      true => (self, other),
      false => (other, self),
    };
    let short_len = short.steps.len();
    let long_len = long.steps.len();
    let match_len = (short.steps.iter()).zip(long.steps.iter()).take_while(|(a, b)| a == b).count();
    // fact: match_len <= short_len <= long_len
    if short_len == match_len && match_len == long_len {
      // implies match_len == short_len == long_len
      match (short.next, long.next) {
        (None, None) => Self::end(short.steps.iter().cloned()),
        (Some(_), None) | (None, Some(_)) => {
          panic!("One of these paths is faulty")
        },
        (Some(s), Some(l)) =>
          Self::branch(short.steps.iter().cloned(), join_maps(s, l, |_, l, r| l.overlay(r))),
      }
    } else if short_len == match_len {
      // implies match_len == short_len < long_len
      // long.steps[0..match_len] is in steps
      // long.steps[match_len] becomes the choice of branch below
      // long.steps[match_len + 1..] is in tail
      let mut conts = short.next.expect("One path ends inside the other");
      let tail_steps = long.steps.split_off(match_len + 1);
      let tail = match long.next {
        Some(n) => Self::branch(tail_steps, n),
        None => Self::end(tail_steps),
      };
      let branch = long.steps[match_len];
      let prev_c = conts.remove(&branch);
      let new_c = if let Some(x) = prev_c { x.overlay(tail) } else { tail };
      conts.insert(branch, new_c);
      Self::branch(short.steps, conts)
    } else {
      // implies match_len < short_len <= long_len
      // steps[0..match_len] is in shared
      // steps[match_len] become the branches below
      // steps[match_len + 1..] is in new_long and new_short
      let new_short_steps = short.steps.split_off(match_len + 1);
      let short_last = short.steps.pop_back().expect("split at n + 1");
      let new_short = Self { next: short.next.clone(), steps: new_short_steps };
      let new_long_steps = long.steps.split_off(match_len + 1);
      let new_long = Self { next: long.next.clone(), steps: new_long_steps };
      Self::branch(short.steps, [(short_last, new_short), (long.steps[match_len], new_long)])
    }
  }

  /// Prepend a step to a path. If it had previously started at a node that is
  /// at the specified step within an Apply clause, it now starts at the Apply.
  ///
  /// This is only valid if the new Apply is **separate** from the previous
  /// root.
  pub fn prepend(&mut self, step: Step) { self.steps.push_front(step); }
}

impl fmt::Display for PathSet {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let step_s = self.steps.iter().copied().map(print_step).join(">");
    match &self.next {
      Some(conts) => {
        let opts = (conts.iter())
          .sorted_unstable_by_key(|(k, _)| k.map_or(0, |n| n + 1))
          .map(|(h, t)| format!("{}>{t}", print_step(*h)))
          .join("|");
        if !step_s.is_empty() {
          write!(f, "{step_s}>")?;
        }
        write!(f, "({opts})")
      },
      None => write!(f, "{step_s}"),
    }
  }
}

impl fmt::Debug for PathSet {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "PathSet({self})") }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_combine() {
    let ps1 = PathSet { next: None, steps: VecDeque::from([Some(2), None]) };
    let ps2 = PathSet { next: None, steps: VecDeque::from([Some(3), Some(1)]) };
    let sum = ps1.clone().overlay(ps2.clone());
    assert_eq!(format!("{sum}"), "(2>f|3>1)");
  }

  fn extend_scaffold() -> PathSet {
    PathSet::branch([None, Some(1), None], [
      (None, PathSet::end([None, Some(1)])),
      (Some(1), PathSet::end([None, Some(2)])),
    ])
  }

  #[test]
  fn test_extend_noclone() {
    let mut ps = extend_scaffold();
    ps.prepend(Some(0));
    assert_eq!(format!("{ps}"), "0>f>1>f>(f>f>1|1>f>2)");
  }
}
