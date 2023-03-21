# List of open-source packages I used

## [thiserror](https://github.com/dtolnay/thiserror)

_License: Apache 2.0 or MIT_

Helps derive `Error` for aggregate errors, although I eventually stopped trying to do so as it was simpler to just treat error types as bags of data about the failure.

## [chumsky](https://github.com/zesterer/chumsky)

_License: MIT_

A fantastic parser combinator that allowed me to specify things like the nuanced conditions under which a float token can be promoted to an uint token in a declarative way. In hindsight passes after tokenization could have been written by hand, tokenized Orchid is not that hard to parse into an AST and it would have probably made some tasks such as allowing `.` (dot) as a token considerably easier.

## [hashbrown](https://github.com/rust-lang/hashbrown)

_License: Apache 2.0 or MIT_

Google's swisstable. Almost perfectly identical to `HashMap` in std, with a couple additional APIs. I use it for the raw entry API which the generic processing step cache requires to avoid unnecessary clones of potentially very large trees.

## [mappable-rc](https://github.com/JakobDegen/mappable-rc)

_License: Apache 2.0 or MIT_

A refcounting pointer which can be updated to dereference to some part of the value it holds similarly to C++'s `shared_ptr`. Using this crate was ultimately a mistake on my part, in early stages of development (early stages of my Rust journey) I wanted to store arbitrary subsections of an expression during macro execution without dealing with lifetimes. Removing all uses of this crate and instead just dealing with lifetimes is on the roadmap.

## [ordered-float](https://github.com/reem/rust-ordered-float)

_License: MIT_

A wrapper around floating point numbers that removes `NaN` from the set of possible values, promoting `<` and `>` to total orderings and `==` to an equivalence relation. Orchid does not have `NaN` because it's a silent error. All operations that would produce `NaN` either abort or indicate the failure in their return type.

## [itertools](https://github.com/rust-itertools/itertools)

_License: Apache 2.0 or MIT_

A utility crate, I use it everywhere.

## [smallvec](https://github.com/servo/references-smallvec)

_License: Apache 2.0 or MIT_

small vector optimization - allocates space for a statically known number of elements on the stack to save heap allocations. This is a gamble since the stack space is wasted if the data does spill to the heap, but it can improve performance massively in hot paths.

## [dyn-clone](https://github.com/dtolnay/dyn-clone)

_License: Apache 2.0 or MIT_

All expressions in Orchid are clonable, and to allow for optimizations, Atoms have control over their own cloning logic, so this object-safe version of `Clone` is used.
