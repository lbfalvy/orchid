use orchid_extension::entrypoint::{extension_main, ExtensionData};
use orchid_std::StdSystem;

pub fn main() { extension_main(ExtensionData { systems: &[&StdSystem] }) }
