# Introduction

Orchid is a lazy, pure functional programming language with an execution model inspired by Haskell and a powerful syntax-level preprocessor for encoding rich DSLs that adhere to the language's core guarantees.

## Immutability

The merits of pure functional code are well known, but I would like to highlight some of them that are particularly relevant in the case of Orchid;

- **Free execution order** The value of any particular subprogram is largely independent of its execution order, so externally defined functions have a lot of liberty in evaluating their arguments. This can ensure that errors are caught early, or even be used to start subtasks in parallel while the rest of the parameters are being collected. With a considerately designed external API, Orchid functions can be reordered and parallelized based on the resources they operate on. This approach can be observed in Rust's Bevy ECS, but Rust is an impure language so it can only guarantee this degree of safety at the cost of great complexity.

- **Self-containment** Arguments to the current toplevel function are all completely self-contained expressions, which means that they can be serialized and sent over the network provided that an equivalent for all atoms and externally defined functions exists on both sides, which makes Orchid a prime query language.
  > **note**
  > Although this is possible using Javascript's `Function` constructor, it is a catastrophic security vulnerability since code sent this way can access all host APIs. In the case of Orchid it is not only perfectly safe from an information access perspective since all references are bound on the sender side and undergo explicit translation, but also from a computational resource perspective since the recipient can apply step limits to the untrusted expression, interleave it with local tasks, and monitor its size and memory footprint.

- **reentrancy** in low reliability environments it is common to run multiple instances of an algorithm in parallel and regularly compare and correct their state using some form of consensus. In an impure language this must be done explicitly and mistakes can result in divergence. In a pure language the executor can be configured to check its state with others every so many steps.

## Laziness

Reactive programming is an increasingly popular paradigm for enabling systems to interact with changing state without recomputing subresults that have not been modified. It is getting popular despite the fact that enabling this kind of programming in classical languages - most notably javascript, where it appears to be the most popular - involves lots of boilerplate and complicated constructs using many many lambda functions. In a lazy language this is essentially the default.

In addition, lazy, pure code lends itself to optimization. Deforestation and TCO are implied and CTFE (or in the case of an interpreted language ahead-of-time function execution) along with a host of other optimizations are more convenient.
