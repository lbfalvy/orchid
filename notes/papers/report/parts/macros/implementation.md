# Implementation

THe optimization of this macro execution algorithm is an interesting challenge with a diverse range of potential optimizations. The current solution is very far from ideal, but it scales to the small experimental workloads I've tried so far and it can accommodate future improvements without any major restructuring.

The scheduling of macros is delegated to a unit called the rule repository, while the matching of rules to a given clause sequence is delegated to a unit called the matcher. Other tasks are split out into distinct self-contained functions, but these two have well-defined interfaces and encapsulate data. Constants are processed by the repository one at a time, which means that the data processed by this subsystem typically corresponds to a single struct, function or other top-level source item.

## keyword dependencies

The most straightforward optimization is to skip patterns that doesn contain tokens that don't appear in the code at all. This is done by the repository to skip entire rules, but not by the rules on the level of individual slices. This is a possible path of improvement for the future.

## Matchers

There are various ways to implement matching. To keep the architecture flexible, the repository is generic over the matcher bounded with a very small trait.

The current implementation of the matcher attempts to build a tree of matchers rooted in the highest priority vectorial placeholder. On each level  The specializations are defined as follows:

- `VecMatcher` corresponds to a subpattern that starts and ends with a vectorial. Each matcher also matches the scalars in between its submatchers, this is not explicitly mentioned.
  
  - `Placeholder` corresponds to a vectorial placeholder with no lower priority vectorials around it

    It may reject zero-length slices but contains no other logic

  - `Scan` corresponds to a high priority vectorial on one side of the pattern with lower priority vectorials next to it.
  
    It moves the boundary - consisting of scalars - from one side to the other

  - `Middle` corresponds to a high priority vectorial surrounded on both sides by lower priority vectorials.

    This requires by far the most complicated logic, collecting matches for its scalar separators on either side, sorting their pairings by the length of the gap, then applying the submatchers on either side until a match is found. This uses copious heap allocations and it's generally not very efficient. Luckily, this kind of pattern almost never appears in practice.

- `ScalMatcher` tests a single token. Since vectorials in subtrees are strictly lower priority than those in parent enclosing sequences `S` and `Lambda` don't require a lot of advanced negotiation logic. They normally appear in sequence, as a their operations are trivially generalizable to a static sequence of them.

- `AnyMatcher` tests a sequence and wraps either a sequence of `ScalMatcher` or a single `VecMatcher` surrounded by two sequences of `ScalMatcher`.