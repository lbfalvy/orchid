use std::fmt::Debug;
use std::ops::Add;
use std::rc::Rc;

use crate::utils::rc_tools::rc_to_owned;
use crate::utils::Side;

/// A branching path selecting some placeholders (but at least one) in a Lambda
/// expression
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PathSet {
  /// The definite steps
  pub steps: Rc<Vec<Side>>,
  /// if Some, it splits. If None, it ends.
  pub next: Option<(Rc<PathSet>, Rc<PathSet>)>,
}

impl PathSet {
  /// Create a path set for more than one target
  pub fn branch(
    steps: impl IntoIterator<Item = Side>,
    left: Self,
    right: Self,
  ) -> Self {
    let steps = Rc::new(steps.into_iter().collect());
    Self { steps, next: Some((Rc::new(left), Rc::new(right))) }
  }

  /// Create a path set for one target
  pub fn end(steps: impl IntoIterator<Item = Side>) -> Self {
    Self { steps: Rc::new(steps.into_iter().collect()), next: None }
  }

  /// Create a path set points to a slot that is a direct
  /// child of the given lambda with no applications. In essence, this means
  /// that this argument will be picked as the value of the expression after an
  /// arbitrary amount of subsequent discarded parameters.
  pub fn pick() -> Self { Self { steps: Rc::new(vec![]), next: None } }
}

impl Debug for PathSet {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for s in self.steps.as_ref() {
      match s {
        Side::Left => write!(f, "L")?,
        Side::Right => write!(f, "R")?,
      }
    }
    match &self.next {
      Some((l, r)) => write!(f, "({l:?}|{r:?})")?,
      None => write!(f, "x")?,
    }
    Ok(())
  }
}

impl Add for PathSet {
  type Output = Self;
  fn add(self, rhs: Self) -> Self::Output {
    Self { steps: Rc::new(vec![]), next: Some((Rc::new(self), Rc::new(rhs))) }
  }
}

impl Add<Side> for PathSet {
  type Output = Self;
  fn add(self, rhs: Side) -> Self::Output {
    let PathSet { steps, next } = self;
    let mut new_steps = rc_to_owned(steps);
    new_steps.insert(0, rhs);
    Self { steps: Rc::new(new_steps), next }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_combine() -> Result<(), &'static str> {
    let ps1 =
      PathSet { next: None, steps: Rc::new(vec![Side::Left, Side::Left]) };
    let ps2 =
      PathSet { next: None, steps: Rc::new(vec![Side::Left, Side::Right]) };
    let sum = ps1.clone() + ps2.clone();
    assert_eq!(sum.steps.as_ref(), &[]);
    let nexts = sum.next.ok_or("nexts not set")?;
    assert_eq!(nexts.0.as_ref(), &ps1);
    assert_eq!(nexts.1.as_ref(), &ps2);
    Ok(())
  }

  fn extend_scaffold() -> PathSet {
    PathSet {
      next: Some((
        Rc::new(PathSet {
          next: None,
          steps: Rc::new(vec![Side::Left, Side::Left]),
        }),
        Rc::new(PathSet {
          next: None,
          steps: Rc::new(vec![Side::Left, Side::Right]),
        }),
      )),
      steps: Rc::new(vec![Side::Left, Side::Right, Side::Left]),
    }
  }

  #[test]
  fn test_extend_noclone() {
    let ps = extend_scaffold();
    let new = ps + Side::Left;
    assert_eq!(new.steps.as_ref().as_slice(), &[
      Side::Left,
      Side::Left,
      Side::Right,
      Side::Left
    ])
  }

  #[test]
  fn test_extend_clone() {
    let ps = extend_scaffold();
    let _anchor = ps.clone();
    let new = ps + Side::Left;
    assert_eq!(new.steps.len(), 4);
  }
}
