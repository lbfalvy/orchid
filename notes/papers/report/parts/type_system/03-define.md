# Define

Define is used to create types and typeclasses. Define is a distinct [[02-parsing#Files|line type]] that has the following form:

```
define = "define" name param* "as" value
param = param_name [ ":" kind ]
kind = clause
param_name = "$" name (without spaces)
value = clause*
```

For an example of a type, here's the definition of a conslist or linked list.
```
define List $T as Y \r. Option (Pair $T r)
```

These aren't macros although they look similar. While macros are processed after parsing and then forgotten, these placeholders are recognized by the language and subject to unification.

It's important to keep in mind that these are nominal types; when something is typed `List int`, it is not assignable to `Option (Pair int (List int))`.

## Typeclasses

Typeclasses are types that describe operations. Very often a typeclass will be a single function, but they can also be sequences of functions.

For an example of a typeclass, here's the definition of Eq, the class of types that can be equality-compared.
```
define Eq $T as $T -> $T -> bool
```

Eq isn't a statement about types as typeclasses commonly are in other languages; instead, it's an operation carried out on a particular type. **Constraints of `Eq` on some generic parameter `T` are expressed as a requirement for the existence of `Eq T` for the given `T`.** As an added benefit, the operations exposed by a typeclass can be unambiguously referenced from the bound name of the typeclass value within the binding.
```
isUnaryGrp := @T. @eq:Eq T. @:Add T T T. \t:T. eq (t + t) t
```

In the above example, the implementation of `Eq` is used directly as a value in the expression. The implementation of `Add` is not used, but it can be assumed that the operator + is translated via macros to a call to some generic function `add` which is constrained on `Add`, so according to the second unification rule in [[#Given (formerly Auto)|Given]] the implementation is forwarded.

## Kinds

Each of the parameters to a nominal type has a kind. Kinds can be thought of as a "type of type", and they ensure that expressions that are used in the type of a value have no unspecified parameters while allowing values to be parametric on parametric types.

### 1. concrete types

`type` is the kind of concrete types. These are the only values in type-space that can stand in the position of a type annotation. Simple types such as `int` as well as fully specified generic types such as `List int` belong to this group.

Kinds aren't inferred from usage; if a type parameter does not have a kind annotation, it is assumed to be `type`.

### 2. generics

Generics or parametric types act like N-ary functions. `type -> type` is the kind of generics with one type parameter, `type -> type -> type` is the kind of generics wiht two type parameters, and so on. `List` for instance is `type -> type`.

Typeclasses applied to simple types also belong in this group. For example, `Eq` from above has kind `type -> type`. `Add` has three generic parameters for left, right and output types, and all of these are concrete types, so its kind is `type -> type -> type -> type`.

### 3. higher-kinded polymorphism

Types that are parametric on parametric types have kinds that are analogous to higher-order functions. Most real-world examples of this group are typeclasses that apply to containers.

`List` has the kind `type -> type`. `Option`, also known as `Maybe` from Haskell also has the same kind, as does `HashMap string`. What's common about all of these is that they have values that can be modified without influencing the overall structure of the containers. In Haskell this capability is encoded in the typeclass `Functor`, but Orchid would probably opt for a more accessible name such as `Mapping`. The kind of this typeclass is `(type -> type) -> type`.
```
define Mapping $C:(type -> type) as @T. @U. C T -> C U
```
