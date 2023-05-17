Myy original inspiration to create Orchid was Haskell. I found the power of lazy evaluation impressive and inspiring and saw its potential in defining zero-cost abstractions with simple data flow. I identified a few key problems that motivated me to build a new language:

**Syntax sugar:** Infix operators in Haskell are defined as any function consisting of non-alphanumeric characters. This produces various rather confusing patterns; ternary operators are placed between their first and second argument, and the ability to use keywords as infix operators and infix operators as prefixes with the use of backticks is a pointless divergence. Other kinds of syntax sugar such as do blocks have a well-defined purpose but often appear as operators in the middle of screen-wide expressions where their purpose is hard to understand and entirely disconnected from the metaphor that brought them to life.

In addition the handling of all syntax sugar is delegated to the compiler. This results in a system that's surprisingly limited when it comes to defining new abstractions, but also requires much greater effort to learn and read than languages with an intentionally limited syntax such as Java.

**Syntax-level metaprogramming:**  [Template Haskell][th1] is Haskell's tool for syntax-level macros. I learned about it after I built Orchid, and it addresses a lot of my problems.

[th1]: ./literature/macros.md

**Type system:** Haskell's type system is very powerful but to be able to represent some really interesting structures it requires a long list of GHC extensions to be enabled which in turn make typeclass implementation matching undecidable and the heuristic rather bad (understandably so, it was clearly not designed for that).

My plan for Orchid was to use Orchid itself as a type system as well; rather than aiming for a decidable type system and then extending it until it inevitably becomes turing-complete [1][2][3], my type-system would be undecidable from the start and progress would point towards improving the type checker to recognize more and more cases.

A description of the planned type system is available in [[type_system/+index|Appendix T]]