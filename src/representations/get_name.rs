use std::sync::atomic::AtomicU64;
use lazy_static::lazy_static;

lazy_static! {
  static ref NEXT_NAME: AtomicU64 = AtomicU64::new(0);
}

pub fn get_name() -> u64 {
  NEXT_NAME.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}