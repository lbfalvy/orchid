# Introduction

Orchid is a lazy, pure functional programming language with an execution model inspired by Haskell and a powerful syntax-level preprocessor for encoding rich DSLs that adhere to the language's core guarantees.

# Immutability

The merits of pure functional code are well known, but I would like to highlight some of them that are particularly relevant in the case of Orchid;

- **Free execution order** The value of any particular subprogram is largely independent of its execution order, so externally defined functions have a lot of liberty in evaluating their arguments. This can ensure that errors are caught early, or even be used to start subtasks in parallel while the rest of the parameters are being collected. With a considerately designed external API, Orchid functions can be reordered and parallelized based on the resources they operate on. This approach can be observed in Rust's Bevy ECS, but Rust is an impure language so it can only guarantee this degree of safety at the cost of great complexity.

- **Self-containment** Arguments to the current toplevel function are all completely self-contained expressions, which means that they can be serialized and sent over the network provided that an equivalent for all atoms and externally defined functions exists on both sides, which makes Orchid a prime query language.
  > **note**
  > Although this is possible using Javascript's `Function` constructor, it is a catastrophic security vulnerability since code sent this way can access all host APIs. In the case of Orchid it is not only perfectly safe from an information access perspective since all references are bound on the sender side and undergo explicit translation, but also from a computational resource perspective since external functions allow the recipient to apply step limits (gas) to the untrusted expression, interleave it with local tasks, and monitor its size and memory footprint.

- **reentrancy** in low reliability environments it is common to run multiple instances of an algorithm in parallel and regularly compare and correct their state using some form of consensus. In an impure language this must be done explicitly and mistakes can result in divergence. In a pure language the executor can be configured to check its state with others every so many steps.

# Laziness

Reactive programming is an increasingly popular paradigm for enabling systems to interact with changing state without recomputing subresults that have not been modified. It is getting popular despite the fact that enabling this kind of programming in classical languages - most notably javascript, where it appears to be the most popular - involves lots of boilerplate and complicated constructs using many many lambda functions. In a lazy language this is essentially the default.

In addition, lazy, pure code lends itself to optimization. Deforestation and TCO are implied and CTFE (or in the case of an interpreted language ahead-of-time function execution) along with a host of other optimizations are more convenient.

# Macros

Left-associative unparenthesized function calls are intuitive in the typical case of just applying functions to a limited number of arguments, but they're not very flexible. Haskell solves this problem by defining a diverse array of syntax primitives for individual use cases such as `do` blocks for monadic operations. This system is fairly rigid. In contrast, Rust enables library developers to invent their own syntax that intuitively describes the concepts the library at hand encodes. In Orchid's codebase, I defined several macros to streamline tasks like defining functions in Rust that are visible to Orchid, or translating between various intermediate representations.

## Generalized kerning

In the referenced video essay, a proof of the Turing completeness of generalized kerning is presented. The proof involves encoding a Turing machine in a string and some kerning rules. The state of the machine is next to the read-write head and all previous states are enumerated next to the tape because kerning rules are reversible. The end result looks something like this:

```
abcbcddddef|1110000110[0]a00111010011101110
```

The rules are translated into kerning rules. For a rule

> in state `a` seeing `0`: new state is `b`, write `1` and go `left`

the kerning rule would look like this (template instantiated for all possible characters):

```
$1 [ 0 ] a equals a < $1 ] b 0
```

Some global rules are also needed, also instantiated for all possible characters in the templated positions

```
$1 $2 <  equals  $2 < $1  unless $1 is |
| $1 <   equals  $1 | >
> $1 $2  equals  $1 > $2  unless $2 is ]
> $1 ]   equals  [ $1 ]
```

What I really appreciate in this proof is how visual it is; based on this, it's easy to imagine how one would go about encoding a pushdown automaton, lambda calculus or other interesting tree-walking procedures. This is exactly why I based my preprocessor on this system.

## Namespaced tokens

Rust macros operate on the bare tokens and therefore are prone to accidental aliasing. Every other item in Rust follows a rigorous namespacing scheme, but macros break this structure, probably because macro execution happens before namespace resolution. The language doesn't suffer too much from this problem, but the relativity of namespacing
limits their potential.

Orchid's substitution rules operate on namespaced tokens. This means that the macros can hook into each other. Consider the following example, which is a modified version of a real rule included in the prelude:

in _procedural.orc_
```orchid
export do { ...$statement ; ...$rest:1 } =10_001=> (
  statement (...$statement) do { ...$rest } 
)
export do { ...$return } =10_000=> (...$return)
export statement (let $_name = ...$value) ...$next =10_000=> (
  (\$_name. ...$next) (...$value)
)
```

in _cpsio.orc_
```orchid
import procedural::statement

export statement (cps $_name = ...$operation) ...$next =10_001=> (
  (...$operation) \$_name. ...$next
)
export statement (cps ...$operation) ...$next =10_000=> (
  (...$operation) (...$next)
)
```

in _main.orc_
```orchid
import procedural::(do, let, ;)
import cpsio::cps

export main := do{
  cps data = readline;
  let a = parse_float data * 2;
  cps print (data ++ " doubled is " ++ stringify a)
}
```

Notice how, despite heavy use of macros, it's never ambiguous where a particular name is coming from. Namespacing, including import statements, is entirely unaffected by the macro system. The source of names is completely invariant.
