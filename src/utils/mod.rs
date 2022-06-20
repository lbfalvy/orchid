mod cache;
mod substack;
pub use cache::Cache;
pub use substack::Substack;

pub fn as_modpath(path: &Vec<String>) -> String {
    path.join("::")
}