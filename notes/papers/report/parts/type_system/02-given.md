### Given (formerly Auto)

`given` bindings have the form `@Name:type. body`. Either the `Name` or the  `:type` part can be optional but at least one is required. The central idea is that wherever a binding is unwrapped by an operation the language attempts to find a value for the name. Bindings are unwrapped in the following situations:

- If the value is used, such as if a generic function is called
- If the value is assigned to something that has a known type which does NOT have a binding

Bindings can be **resolved** in a couple ways:

1. If the name appears in the type of any value, type unification provides a solution
2. If the binding has a type and the point of unwrapping is within the body of a binding with an **assignable** type, the value of that binding is forwarded
3. If none of the above options yield any success and the binding has a type, the value of the single suitable `impl` according to the [[04-impl#Matching rules|impl matching rules]] is used

If none of the above options are successful, resolution fails.

It is possible to store values with bindings in typed datastructures without resolving the binding, for example `List @T. @:Eq T. (T -> Option bool)` would represent a `List` of functions that take any equality-comparable value and return an optional boolean.

Bindings can be used to represent generics. In the above example, `@T. ...` is a generic parameter. It translates to the clause "given a type T, ...". Its value will probably be decided by the function's argument.

Bindings can also be used to represent constraints. In the above example, `@:Eq T. ...` is a constraint, which translates to the clause "given an instance of `Eq T`, ...". Its value will have to be decided by an existing `Eq` constraint if the caller is also generic over `T`, or an `impl` of `Eq` if the function is called on a value of a concrete type or if the caller does not have the `Eq` constraint.