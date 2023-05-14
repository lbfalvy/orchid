# Open-source packages Orchid depends on

## [thiserror](https://github.com/dtolnay/thiserror)

_License: Apache 2.0 or MIT_

Helps derive `Error` for aggregate errors.

I eventually stopped trying to do this as it was simpler to just treat error types as bags of data about the failure, but some parts of the codebase still use it and it doesn't really cause any problems.

## [chumsky](https://github.com/zesterer/chumsky)

_License: MIT_

A fantastic parser combinator that allowed specifying nuanced decisions in a declarative way, such as whether a given float token can be promoted to an uint token.

In hindsight passes after tokenization could have been written by hand, tokenized Orchid is not that hard to parse into an AST and it would have probably made some tasks such as allowing `.` (dot) as a token considerably easier.

## [hashbrown](https://github.com/rust-lang/hashbrown)

_License: Apache 2.0 or MIT_

Google's swisstable implementation. Almost perfectly identical to `std::collections::HashMap`, with minor differences.

One of its greatest feats is support for the raw entry API which enables resolving entries using a hash and an equality lambda. This is used both by the interner to avoid many clones and allocations and by the generic processing step cache to avoid unnecessary clones of potentially very large trees. This API is experimentally available in the native hashmap too.

Its other advantage over `std::collections::HashMap` is that its default hashing function is AHash which is said to be faster than the standard variant's default SipHash. I don't have benchmarks to back this up but since it was already in the codebase for the raw entry API I opted to use it everywhere.

## [ordered-float](https://github.com/reem/rust-ordered-float)

_License: MIT_

A wrapper around floating point numbers that removes `NaN` from the set of possible values, promoting `<` and `>` to total orderings and `==` to an equivalence relation. Orchid does not have `NaN` because it's a silent error which conflicts with the "let it crash" philosophy borrowed from Elixir. All operations that would produce `NaN` either abort or indicate the failure in their return type.

## [itertools](https://github.com/rust-itertools/itertools)

_License: Apache 2.0 or MIT_

A fundamental utility crate for Rust's iterators, it's impossible to enumerate its uses.

## [smallvec](https://github.com/servo/references-smallvec)

_License: Apache 2.0 or MIT_

small vector optimization - allocates space for a statically known number of elements on the stack to save heap allocations. This is a gamble since the stack space is wasted if the data does spill to the heap, but it can improve performance massively in hot paths.

I used it for optimizations in the key-value store the type system used to store 

## [dyn-clone](https://github.com/dtolnay/dyn-clone)

_License: Apache 2.0 or MIT_

All expressions in Orchid are clonable, and to allow for optimizations, Atoms have control over their own cloning logic, so this object-safe version of `Clone` is used.

# Packages no longer used

## [mappable-rc](https://github.com/JakobDegen/mappable-rc)

A refcounting pointer which can be updated to dereference to some part of the value it holds similarly to C++'s `shared_ptr`.

Using this crate was ultimately a mistake on my part, in early stages of development (early stages of my Rust journey) I wanted to store arbitrary subsections of an expression during macro execution without dealing with lifetimes. It was removed in the latest version.

## [lasso](https://github.com/Kixiron/lasso)

A very popular string interner, used for interning both strings and base64 encoded data

## [base64](https://github.com/marshallpierce/rust-base64)

Enable interning non-string data

## [static_init](https://gitlab.com/okannen/static_init)

Enable interning magic strings ahead-of-time in functions that don't have access to the interner.

I thought that this actually runs static initializers on startup as it's advertised in the readme