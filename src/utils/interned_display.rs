use std::fmt::Display;

use lasso::RodeoResolver;

pub trait InternedDisplay {
  fn fmt(&self,
    f: &mut std::fmt::Formatter<'_>,
    rr: RodeoResolver
  ) -> std::fmt::Result;
}

impl<T> InternedDisplay for T where T: Display {
  fn fmt(&self,
      f: &mut std::fmt::Formatter<'_>,
      rr: RodeoResolver
    ) -> std::fmt::Result {
      <Self as Display>::fmt(&self, f)
  }
}