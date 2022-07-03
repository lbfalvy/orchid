mod cache;
mod substack;
mod result_iter_collect;
pub use cache::Cache;
pub use substack::Substack;
pub use result_iter_collect::result_iter_collect;

pub type BoxedIter<'a, T> = Box<dyn Iterator<Item = T> + 'a>;