# Type system

This is a description of the type system originally designed for Orchid which never reached the MVP stage.

At the core the type system consists of three concepts:

- `define` creates nominal types, which also act as typeclasses. This may be very confusing but it will make more sense later.
- `impl` provides instances of typeclasses
- a universal parametric construct that serves as both a `forall` (or generic) and a `where` (or constraint). This was temporarily named `auto` but is probably more aptly described by the word `given`.

## Unification

The backbone of any type system is unification. In this case, this is an especially interesting question because the type expressions are built with code and nontermination is outstandingly common.

The unification process uses Hindley-Milner unification as a primitive. It attempts to find an MGU within a constant N steps of reduction. In every step, the candidates are compared using HM, and if it fails, branches are created for each transformation available in the tree. All branches reference the previous step. Valid transformations are

- $\beta$-reduction
- Replacing a subtree that is syntactically equivalent to a tree it was produced by with a call to the Y combinator.

This algorithm is prone to state explosion, but because it does not need to solve extremely complex problems but rather many many very small ones, good caching can probably solve most issues.