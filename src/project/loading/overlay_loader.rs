use super::{Loader, LoadingError};

pub fn overlay_loader(mut base: impl Loader, mut overlay: impl Loader) -> impl Loader {
  move |path: &[&str]| match overlay.load(path) {
    ok@Ok(_) => ok,
    e@Err(LoadingError::IOErr(_)) => e,
    Err(_) => base.load(path)
  }
}

#[macro_export]
macro_rules! overlay_loader {
  ($left:expr, $right:expr) => {
    overlay_loader($left, $right)
  };
  ($left:expr, $mid:expr, $($rest:expr),+) => {
    overlay_loader($left, overlay_loader!($mid, $($rest),+))
  };
}