## Type definitions

A new type can be created with the define expression, which associates a templated expression of
type `type` with a name and a template. The name allocated in this fashion is always representedas
an Atom of type `type` or some function that eventually returns `type`. The kind of the template
parameters is always inferred to be `type` rather than deduced from context.

The following type definition

```orc
define Cons $T as loop \r. Option (Pair $T r)
```

results in these conditions:

- (Cons Int) is not assignable to @T. Option T, or any other type expression that its
  definitions would be assignable to, and vice versa.
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

## Typeclasses

Typeclasses and types use the same define syntax. In fact, much like a type is nothing but a
distinguished instance of the underlying type with added meaning and constraints, a typeclass is
nothing but a distinguished instance of the underlying function (or collection of functions) with
added meaning and constraints. A typeclass definition is therefore perfectly identical to a type
definition:

```
define Add $T $U $R as $T -> $U -> $R
```

It is clear that the definition of this type would match many, many functions, including
multiplication, so functions that should be considered addition are [impls](./impls.md) of the
typeclass Add.
