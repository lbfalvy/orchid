use never::Never;

/// Fallible variant of [Add]
pub trait Combine: Sized {
  type Error;

  fn combine(self, other: Self) -> Result<Self, Self::Error>;
}

impl Combine for Never {
  type Error = Never;
  fn combine(self, _: Self) -> Result<Self, Self::Error> { match self {} }
}

impl Combine for () {
  type Error = Never;
  fn combine(self, (): Self) -> Result<Self, Self::Error> { Ok(()) }
}
