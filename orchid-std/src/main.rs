use std::mem;
use std::rc::Rc;

use orchid_extension::entrypoint::{ExtensionData, extension_main_logic};
use orchid_std::StdSystem;
use tokio::task::{LocalSet, spawn_local};

#[tokio::main(flavor = "current_thread")]
pub async fn main() {
	LocalSet::new()
		.run_until(async {
			let data = ExtensionData::new("orchid-std::main", &[&StdSystem]);
			extension_main_logic(data, Rc::new(|fut| mem::drop(spawn_local(fut)))).await;
		})
		.await
}
