use orchid_api_traits::Coding;

pub trait AsApi: Sized {
  type Api: Sized;
  type Ctx<'a>;
  fn to_api(&self, ctx: Self::Ctx<'_>) -> Self::Api;
  fn into_api(self, ctx: Self::Ctx<'_>) -> Self::Api { self.to_api(ctx) }
  fn from_api_ref(api: &Self::Api, ctx: Self::Ctx<'_>) -> Self;
  fn from_api(api: Self::Api, ctx: Self::Ctx<'_>) -> Self { Self::from_api_ref(&api, ctx) }
}
