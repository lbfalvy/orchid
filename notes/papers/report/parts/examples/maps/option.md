# Option

This example uses a lot of lists of known length, but with the introduction of `list::get` a lot of `option`s are added to the flow of logic. A way to mark impossible branches is needed.

This is handled using a new external function called `std::panic`. Since Orchid is a sandboxed language this doesn't actually cause a Rust panic, instead it produces a dedicated ExternError when it's first reduced. Using this, `option::unwrap` is trivial to define.