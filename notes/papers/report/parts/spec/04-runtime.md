## Runtime

Orchid is evaluated lazily. This means that everything operates on unevaluated expressions. This has the advantage that unused values never need to be computed, but it also introduces a great deal of complexity in interoperability.

### Gas

The executor supports an optional gas parameter to limit the number of normalization steps taken. Once an Orchid program reaches an inert state, it is either an external item, a literal, or a lambda function.

### external API

In order to do anything useful, Orchid provides an API for defining clauses that have additional behaviour implemented in Rust. Basic arithmetic is defined using these.

#### Atomic

atomics are opaque units of foreign data, with the following operations:

- a function for reduction that behaves like the interpreter's `run` function
- attempt to downcast to a concrete type

Atomics can be used to represent processes. Given enough processing cycles, these return a different clause.

They can also be used to wrap data addressed to other external code. This category of atomics reports inert at all times, and relies on the downcasting API to interact with ExternFn-s.

It's possible to use a combination of these for conditional optimizations - for instance, to recognize chains of processes that can be more efficiently expressed as a single task.

#### ExternFn

external functions can be combined with another clause to form a new clause. Most of the time, this new clause would be an Atomic which forwards processing to the arguments until they can't be normalized any further, at which point it either returns an ExternFn to take another argument or executes the operation associated with the function and returns a value.

Because this combination of operations is so common, several macros are provided to streamline it.

It is always a logic error to normalize expressions outside an `interpreter::run` (or `Atomic::run`) call, or to expect an expression to be of any particular shape without ensuring that `interpreter::run` reported inert in the past.

All functions including external ones are assumed to be pure, and the executor uses opportunistic caching to avoid re-evaluating subexpressions, so continuation-passing style cannot be used to encode side effects. An alternative system for this purpose is being developed, but for the time being the previous CPS functions are still available in the standard library. Each print expression will be printed at least once for each qualitatively distinct argument it is applied to.
