mod cache;
mod substack;
mod side;
mod merge_sorted;
mod sorted_pairs;
mod unwrap_or_continue;
pub use cache::Cache;
pub use substack::Stackframe;
pub use side::Side;
pub use merge_sorted::merge_sorted;

pub type BoxedIter<'a, T> = Box<dyn Iterator<Item = T> + 'a>;