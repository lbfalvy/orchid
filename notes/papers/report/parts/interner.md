## Interner

To fix a very serious performance problem with the initial POC, all tokens and all namespaced names in Orchid are interned.

String interning is a fairly simple optimization, the core idea is to replace strings with an ID unique to the data so that equality comparison can be executed on those IDs in place instead of having to fetch the data from possibly an uncached memory location and compare it character by character. This optimization is so popular that most high-level programming languages with immutable strings automatically do it for string literals, and it allows a lot of otherwise intolerably string-heavy systems such as Javascript's string-map objects to be not only functional but quite performant.

For the sake of simplicity in Rust it is usually done by replacing Strings with a NonZeroU32 (or some other size). This system is very easy to understand and manage since the user doesn't have to deal with lifetimes, but it has a weakness wherein in order to print or in any other way interact with the strings themselves one needs access to the interner object itself. This is perhaps the most significant code smell in Orchid, essentially every function takes a parameter that references the interner.

Interning is of course not limited to strings, but one has to be careful in applying it to distinct concepts as the lifetimes of every single interned thing are tied together, and sometimes the added constraints and complexity aren't worth the performance improvements. Orchid's interner is completely type-agnostic so that the possibility is there. The interning of Orchid string literals is on the roadmap hawever.

### Initial implementation

Initially, the interner used Lasso, which is an established string interner with a wide user base.

#### Singleton

A string interner is inherently a memory leak, so making it static would have likely proven problematic in the future. At the same time, magic strings should be internable by any function with or without access to the interner since embedders of Orchid should be able to reference concrete names in their Rust code conveniently. To get around these constraints, the [[oss#static_init|static_init]] crate was used to retain a global singleton instance of the interner and intern magic strings with it. After the first non-static instance of the interner is created, the functions used to interact with the singleton would panic. I also tried using the iconic lazy_static crate, but unfortunately it evaluates the expressions upon first dereference which for functions that take an interner as parameter is always after the creation of the first non-static interner.

#### The Interner Trait

The interner supported exchanging strings or sequences of tokens for tokens. To avoid accidentally comparing the token for a string with the token for a string sequence, or attempting to resolve a token referring to a string sequence as a string, the tokens have a rank, encoded as a dependent type parameter. Strings are exchanged for tokens of rank 0, and sequences of tokens of rank N are exchanged for tokens of rank N+1.

#### Lasso shim

Because the type represented by a token is statically guaranteed, we can fearlessly store differently encoded values together without annotation. Thanks to this, strings can simply be forwarded to lasso without overhead. Token sequences are more problematic because the data is ultimately a sequence of numbers and we can't easily assert that they will constitute a valid utf8 string. My temporary solution was to encode the binary data in base64.

### Revised implementation

The singleton ended completely defunct because `static_init` apparently also evaluates init expressions on first dereference. Fixing this issue was a good occasion to come up with a better design for the interner.

#### monotype

The logic for interning itself is encapsulated by a `monotype` struct. This stores values of a single homogenous type using a hashmap for value->token lookup and a vector for token->value lookup. It is based on, although considerably simpler than Lasso.

#### polytype

The actual Interner stores a `HashMap<typeid, Box<dyn Any>>`, which is essentially a store of values of unique type keyed by the type. The values in this case are monotype interners.

Unlike the naiive initial implementation, this version also operates on references, so interning and externing values causes no unnecessary copying and heap allocations.

### The InternedDisplay Trait

For refined error reporting most structures derive `Debug` and also implement `Display`. In most cases where the structure at hand describes code of some kind, `Display` attempts to print a fragment of valid code. With every name in the codebase interned this is really difficult because interner tokens can't be resolved from `Display` implementations. To solve this, a new trait was defined called `InternedDisplay` which has the same surface as `Display` except for the fact that `fmt`'s mirror image also takes an additional reference to Interner. The syntax sugar for string formatting is in this way unfortunately lost, but the functionality and the division of responsibilities remains.