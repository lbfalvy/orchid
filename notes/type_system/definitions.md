## Type definitions

```orc
define Cons as \T:type. loop \r. Option (Pair T r)
```

Results in
- (Cons Int) is not assignable to @T. Option T
- An instance of (Cons Int) can be constructed with `categorise @(Cons Int) (some (pair 1 none))`
  but the type parameter can also be inferred from the expected return type
- An instance of (Cons Int) can be deconstructed with `generalise @(Cons Int) numbers`
  but the type parameter can also be inferred from the argument

These inference rules are never reversible

```orc
categorise :: @T:type. (definition T) -> T
generalise :: @T:type. T -> (definition T)
definition :: type -> type -- opaque function
```

## Unification

The following must unify:

```orc
@T. @:Add T T T. Mult Int T T
Mult Int (Cons Int) (Cons Int)
```

### Impls for types

Impls for types are generally not a good idea as autos with types like Int can
often be used in dependent typing to represent eg. an index into a type-level conslist to be
deduced by the compiler, and impls take precedence over resolution by unification.
