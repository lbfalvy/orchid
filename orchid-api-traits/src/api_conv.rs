pub trait ApiEquiv {
  type Api;
}

pub trait ToApi: Sized + ApiEquiv {
  type Ctx;
  fn to_api(&self, ctx: &mut Self::Ctx) -> Self::Api;
  fn into_api(self, ctx: &mut Self::Ctx) -> Self::Api { self.to_api(ctx) }
}

pub trait FromApi: ApiEquiv {
  type Ctx;
  fn from_api(api: &Self::Api, ctx: &mut Self::Ctx) -> Self;
}

/// This is the weakest kind of conversion possible;
/// By holding a reference to the source type, you can provide a reference to the target type.
/// Unlike Into, the target type may hold references into the source,
/// but unlike AsRef, it doesn't have to be fully contained in the source.
/// The resulting object is stackbound so its utility is very limited.
pub trait ProjectionMut<T> {
  fn with_built<R>(&mut self, cb: impl FnOnce(&mut T) -> R) -> R;
}
impl<T> ProjectionMut<T> for T {
  fn with_built<R>(&mut self, cb: impl FnOnce(&mut T) -> R) -> R { cb(self) }
}
