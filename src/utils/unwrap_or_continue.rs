#[macro_export]
macro_rules! unwrap_or_continue {
    ($m:expr) => {
        { if let Some(res) = ($m) {res} else {continue} }
    }
}