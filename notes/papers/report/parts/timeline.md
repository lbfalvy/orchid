# Timeline

I started working on a functional language in February 2022. I was mostly inspired by Haskell and Rust, I wanted to create a lazy, pure language with a simple rigid syntax tree like Rust that would support macros. By the end of August, I had a proof-of-concept implementation of the macro executor, just enough to test my ideas.

This is also when I came up with the name. I read an article about how orchids don't so much grow on, but rather together with mangrove trees and influence the trees to produce patterns beneficial to them while also killing fungi and extending the tree's capacity for photosynthesis.

Having tested that my idea could work, at the start of the academic year I switched to the type system. When the project synopsis was written, I imagined that the type system would be an appropriately sized chunk of the work for a final year project; its title was "Orchid's Type System".

Around the end of November I had researched enough type theory to decide what kind of type system I would want. My choice was advised by a number of grievances I had with Typescript such as the lack of higher-kinded types which comes up surprisingly often[4] in Javascript, lack of support for nominal types and the difficulty of using dependent types. I appreciated however the powerful type transformation techniques.

However, building a type system proved too difficult; on February 23 I decided to cut my losses and focus on building an interpreter. The proof-of-concept interpreter was finished on March 10, but the macro executor was still using the naiive implementation completed over the summer so it would take around 15 seconds to load an example file of 20 lines, and a range of other issues cropped up as well cumulatively impacting every corner of the codebase. A full rewrite was necessary.

The final, working implementation was completed on May 8, this one uses token interning, starts up almost instantly and memoizes expressions by origin. This feature is implemented because it was very straightforward, but it actually conflicts with the pre-existing IO capabilities which still use continuation passing, so IO in a loop is actually impossible.

## Immediate future

The first order of business is to extend the standard library to a basic usable level, I'd like to try adding Elixir-like protocols with multiple type parameters, and some kind of IO support, perhaps mimicking algebraic effects. After that I would like to develop the embedding interface, as I hope to use Orchid in numerous future projects.
