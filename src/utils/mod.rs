mod cache;
mod substack;
pub use cache::Cache;
pub use substack::Substack;

pub type BoxedIter<'a, T> = Box<dyn Iterator<Item = T> + 'a>;