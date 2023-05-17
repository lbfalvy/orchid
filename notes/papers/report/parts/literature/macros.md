https://doc.rust-lang.org/reference/macros-by-example.html

Rust's macro system was both an invaluable tool and an example while defining Orchid's macros.

Rust supports declarative macros in what they call "macros by example". These use a state machine-like simplistic parser model to match tokens within the strictly bounded parameter tree. Most notably, Rust's declarative macros don't support any kind of backtracking. They are computationally equivalent to a finite state machine.

---

https://wiki.haskell.org/Template_Haskell

Template haskell is haskell's macro system that I learned about a little bit too late.

Throughout this project I was under the impression that Haskell didn't support macros at all, as I didn't discover template haskell until very recently. It is a fairly powerful system, although like Rust's macros their range is bounded, so they can hardly be used to define entirely new syntax. There also seem to be a lot of technical limitations due to this feature not being a priority to GHC.

---

https://www.open-std.org/jtc1/sc22/wg21/docs/papers/2019/p0707r4.pdf
https://www.youtube.com/watch?v=4AfRAVcThyA

This paper and the corresponding CppCon talk motivated me to research more natural, integrated forms of metaprogramming.

The paper describes a way to define default behaviour for user-defined groups of types extending the analogy of enums, structs and classes using a compile-time evaluated function that processes a parameter describing the contents of a declaration. It is the first metaprogramming system I encountered that intended to write meta-programs entirely inline, using the same tools the value-level program uses.

This eventually lead to the concept of macros over fully namespaced tokens.

---

https://www.open-std.org/jtc1/sc22/wg21/docs/papers/2021/p2392r0.pdf
https://www.youtube.com/watch?v=raB_289NxBk

This paper and the corresponding CppCon talk demonstrate a very intersting syntax extension to C++.

C++ is historically an object-oriented or procedural language, however in recent standards a significant movement towards declarative, functional patterns manifested. This paper in particular proposes a very deep change to the syntax of the language, an entirely new class of statements that simultaneously bind an arbitrary number of names and return a boolean, that may result in objects being constructed, partially moved and destroyed. The syntax extensions appear very fundamental and yet quite convenient, but what little C++ has in terms of local reasoning suffers. This was interesting and inspirational to me because it demonstrated that considerate syntax extensions can entirely redefine a language, while also reminding about C++'s heritage.

