// FILE SEPARATION BOUNDARY
//
// Collect all operators accessible in each file, parse the files with
// correct tokenization, resolve glob imports, convert expressions to
// refer to tokens with (local) absolute path, and connect them into a
// single tree.
//
// The module checks for imports from missing modules (including
// submodules). All other errors must be checked later.
//
// Injection strategy:
// Return all items of the given module in the injected tree for
// `injected` The output of this stage is a tree, which can simply be
// overlaid with the injected tree

mod add_prelude;
mod build_tree;
mod collect_ops;
mod normalize_imports;
mod parse_file;
mod prefix;

pub use build_tree::{build_tree, split_path};
pub use collect_ops::InjectedOperatorsFn;
