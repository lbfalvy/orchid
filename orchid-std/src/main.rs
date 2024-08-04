use orchid_extension::entrypoint::ExtensionData;
use orchid_std::StdSystem;

pub fn main() { ExtensionData::new("orchid-std::main", &[&StdSystem]).main() }
