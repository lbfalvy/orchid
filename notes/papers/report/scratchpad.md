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

One major grievance of mine with Haskell is that its syntax isn't accessible. Even after understanding the rules, getting used to reading it takes considerable time. On the other hand, I really like the way Rust enables library developers to invent their own syntax that intuitively describes the concepts the library at hand encodes. In Orchid's codebase, I defined several macros to streamline tasks like defining functions in Rust that are visible to Orchid.

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

I found two major problems with C and Rust macros which vastly limit their potential. They're relatively closed systems, and prone to aliasing. Every other item in Rust follows a rigorous namespacing scheme, but the macros break this seal, I presume the reason is that macro execution happens before namespace resolution.

Orchid's macros - substitution rules - operate on namespaced tokens. This means that users can safely give their macros short and intuitive names, but it also means that the macros can hook into each other. Consider for example the following example, which is a slightly modified version of a
real rule included in the prelude:

in _procedural.or_
```orchid
export do { ...$statement ; ...$rest:1 } =10_001=> (
  statement (...$statement) do { ...$rest } 
)
export do { ...$return } =10_000=> (...$return)
export statement (let $_name = ...$value) ...$next =10_000=> (
  (\$_name. ...$next) (...$value)
)
```

in _cpsio.or_
```orchid
import procedural::statement

export statement (cps $_name = ...$operation) ...$next =10_001=> (
  (...$operation) \$_name. ...$next
)
export statement (cps ...$operation) ...$next =10_000=> (
  (...$operation) (...$next)
)
```

in _main.or_
```orchid
import procedural::(do, let, ;)
import cpsio::cps

export main := do{
  cps data = readline;
  let a = parse_float data * 2;
  cps print (data ++ " doubled is " ++ stringify a)
}
```

