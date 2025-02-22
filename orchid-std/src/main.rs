use std::mem;
use std::rc::Rc;

use orchid_extension::entrypoint::ExtensionData;
use orchid_extension::tokio::tokio_main;
use orchid_std::StdSystem;
use tokio::task::{LocalSet, spawn_local};

#[tokio::main(flavor = "current_thread")]
pub async fn main() { tokio_main(ExtensionData::new("orchid-std::main", &[&StdSystem])).await }
