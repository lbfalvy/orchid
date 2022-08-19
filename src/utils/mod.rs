mod cache;
mod substack;
mod side;
mod merge_sorted;
mod unwrap_or_continue;
pub mod iter;
pub use cache::Cache;
use mappable_rc::Mrc;
pub use substack::Stackframe;
pub use side::Side;
pub use merge_sorted::merge_sorted;
pub use iter::BoxedIter;

pub fn mrc_derive<T: ?Sized, P, U: ?Sized>(m: &Mrc<T>, p: P) -> Mrc<U>
where P: for<'a> FnOnce(&'a T) -> &'a U {
    Mrc::map(Mrc::clone(m), p)
}

pub fn mrc_try_derive<T: ?Sized, P, U: ?Sized>(m: &Mrc<T>, p: P) -> Option<Mrc<U>>
where P: for<'a> FnOnce(&'a T) -> Option<&'a U> {
    Mrc::try_map(Mrc::clone(m), p).ok()
}

pub fn to_mrc_slice<T>(v: Vec<T>) -> Mrc<[T]> {
    Mrc::map(Mrc::new(v), |v| v.as_slice())
}

pub fn collect_to_mrc<I>(iter: I) -> Mrc<[I::Item]> where I: Iterator {
    to_mrc_slice(iter.collect())
}

pub fn mrc_derive_slice<T>(mv: &Mrc<Vec<T>>) -> Mrc<[T]> {
    mrc_derive(mv, |v| v.as_slice())
}
