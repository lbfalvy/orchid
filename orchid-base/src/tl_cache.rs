#[macro_export]
macro_rules! tl_cache {
	($ty:ty : $expr:expr) => {{
		thread_local! {
			static V: $ty = $expr;
		}
		V.with(|v| v.clone())
	}};
}
