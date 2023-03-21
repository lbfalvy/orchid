# Runtime

Orchid is evaluated lazily. This means that everything operates on unevaluated expressions. This has the advantage that unused values never need to be computed, but it also introduces a great deal of complexity in interoperability.

## Execution mode

The executor supports step-by-step execution, multiple steps at once, and running an expression to completion. Once an Orchid program reaches a nonreducible state, it is either an external item, a literal, or a lambda function.

## external API

In order to do anything useful, Orchid provides an API for defining clauses that have additional behaviour implemented in Rust. Basic arithmetic is defined using these.

### Atomic

atomics are opaque units of foreign data, with the following operations:

- functions for the same three execution modes the language itself supports
- downcasting to a concrete type

Atomics can be used to represent processes. Given enough processing cycles, these return a different clause.

They can also be used to wrap data addressed to other external code. This category of atomics reports nonreducible at all times, and relies on the downcasting API to interact with ExternFn-s.

It's possible to use a combination of these for conditional optimizations - for instance, to recognize chains of processes that can be more efficiently expressed as a single task.

### ExternFn

external functions can be combined with another clause to form a new clause. Most of the time, this new clause would be an Atomic which forwards processing to the arguments until they can't be normalized any further, at which point it either returns an ExternFn to take another argument or executes the operation associated with the function and returns.

Because this combination of operations is so common, several macros are provided to streamline it.

Sometimes, eg. when encoding effectful functions in continuation passing style, an ExternFn returns its argument without modification. It is always a logic error to run expressions outside a run call, or to expect an expression to be of any particular shape without ensuring that run returned nonreducible in the past.
