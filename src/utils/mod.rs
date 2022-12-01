mod cache;
mod substack;
mod side;
mod merge_sorted;
mod unwrap_or;
pub mod iter;
mod bfs;
mod unless_let;
mod string_from_charset;
mod for_loop;
mod protomap;

pub use cache::Cache;
use mappable_rc::Mrc;
pub use substack::Stackframe;
pub use side::Side;
pub use merge_sorted::merge_sorted;
pub use iter::BoxedIter;
pub use string_from_charset::string_from_charset;
pub use protomap::ProtoMap;

pub fn mrc_derive<T: ?Sized, P, U: ?Sized>(m: &Mrc<T>, p: P) -> Mrc<U>
where P: for<'a> FnOnce(&'a T) -> &'a U {
    Mrc::map(Mrc::clone(m), p)
}

pub fn mrc_try_derive<T: ?Sized, P, U: ?Sized>(m: &Mrc<T>, p: P) -> Option<Mrc<U>>
where P: for<'a> FnOnce(&'a T) -> Option<&'a U> {
    Mrc::try_map(Mrc::clone(m), p).ok()
}

pub fn mrc_empty_slice<T>() -> Mrc<[T]> {
    mrc_derive_slice(&Mrc::new(Vec::new()))
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

pub fn one_mrc_slice<T>(t: T) -> Mrc<[T]> {
    Mrc::map(Mrc::new([t; 1]), |v| v.as_slice())
}

pub fn mrc_to_iter<T>(ms: Mrc<[T]>) -> impl Iterator<Item = Mrc<T>> {
    let mut i = 0;
    std::iter::from_fn(move || if i < ms.len() {
        let out = Some(mrc_derive(&ms, |s| &s[i]));
        i += 1;
        out
    } else {None})
}

pub fn mrc_unnest<T>(m: &Mrc<Mrc<T>>) -> Mrc<T> {
    Mrc::clone(m.as_ref())
}

pub fn mrc_slice_to_only<T>(m: Mrc<[T]>) -> Result<Mrc<T>, ()> {
    Mrc::try_map(m, |slice| {
        if slice.len() != 1 {None}
        else {Some(&slice[0])}
    }).map_err(|_| ())
}

pub fn mrc_slice_to_only_option<T>(m: Mrc<[T]>) -> Result<Option<Mrc<T>>, ()> {
    if m.len() > 1 {return Err(())}
    Ok(Mrc::try_map(m, |slice| {
        if slice.len() == 0 {None}
        else {Some(&slice[0])}
    }).ok())
}