https://www.unison-lang.org/learn/fundamentals/abilities/

An excellent description of algebraic effects that lead me to understand how they work and why they present an alternative to monads.

Algebraic effects essentially associate a set of special types representing families of requests to a function that it may return other than its own return type. Effects usually carry a thunk or function to enable resuming normal processing, and handlers usually respond to the requests represented by the effects by implementing them on top of other effects such as IO. The interesting part to me is that all of this is mainly just convention, so algebraic effects provide type system support for expressing arbitrary requests using CPS.

Although Orchid doesn't have a type system, CPS is a straightforward way to express side effects

---

https://github.com/zesterer/tao

The first place where I encountered algebraic effects, otherwise a very interesting language that I definitely hope to adopt features from in the future

Tao is made by the same person who created Chumsky, the parser combinator used in Orchid. It demonstrates a lot of intersting concepts, its pattern matching is one of a kind. The language is focused mostly on static verification and efficiency neither of which are particularly strong points of Orchid, but some of its auxiliary features are interesting to an untyped, interpreted language too. One of these is generic effects.

---

https://wiki.haskell.org/All_About_Monads#A_Catalog_of_Standard_Monads

Originally, I intended to have dedicated objects for all action types, and transformations similar to Haskell's monad functions.

A monad is a container that can store any type and supports three key operations:

1. Constructing a new instance of the container around a value
2. Flattening an instance of the container that contains another instance of it into a single container of the inner nested value
3. applying a transformation to the value inside the container that produces a different type

The defining characteristic of monads is that whether and when the transformations are applied is flexible since information can't easily leave the monad.

This system is extremely similar to effects, and at least in an untyped context they're essentially equally powerful. I opted for effects because their defaults seem more sensible.