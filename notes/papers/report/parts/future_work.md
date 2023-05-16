# Future work

## Standard library

There are a few libraries I would like to implement in the future to demonstrate various uses of the language

### Macro error reporting

When describing syntax transformations with Orchid macros, it's fairly easy to make assertions about the stages during which given tokens should exist in the code in terms of the lower and upper bound of the currently active priority number. When these assertions fail, the consequences can be very difficult to debug since a partially transformed syntax tree with all sorts of carriages around conforms to neither the public API of any library nor the core language and lambda calculus. This problem can be addressed with guard rules and bubbling errors. To demonstrate, consider this module:

```
-- in client library
import std::macro_error::missing_token

-- what this carriage does is not relevant to the example, focus on the priority numbers
start_carriage $init =100=> carriage ($init)
carriage ($state) $f =10_001=> carriage (($f $state))
carriage ($state) stop_carriage =10_002=> $state

-- report the suspected reason why this carriage did not get consumed
carriage ($state) =0=> (missing_token stop_carriage ($state))

export ::(start_carriage, stop_carriage)
```

```
-- in std::macro_error

-- convert various errors to uniform format
export (missing_token $token ..$details) =1_000_000=> (bubbling_error
  "{} was not found" ($token) (..$details)
)

-- forward error upwards
(..$_prefix (bubbling_error ...$args) ..$_suffix) =1_000_001=> (bubbling_error ...$args)
[..$_prefix (bubbling_error ...$args) ..$_suffix] =1_000_001=> (bubbling_error ...$args)
{..$_prefix (bubbling_error ...$args) ..$_suffix} =1_000_001=> (bubbling_error ...$args)
```

With this, various options are available for displaying the error:

1. bubbling_error could be a magic token that always causes the macro executor to format and print the following string
2. bubbling_error could be defined as a function to raise an error when the problematic function is called. This only supports the (in my opinion, insufficient) level of error reporting Python provides for syntax errors
3. bubbling_error could be left undefined, the runtime could expose processed functions that contained undefined names after macro execution, and dev tooling could parse bubbling_error out of this data.

### Extensible structural(?) pattern matching

Since all tokens are namespaced, complicated protocols can be defined between libraries for dispatching macro resolution. I would like to make use of this to build a pattern matching library that resolves patterns to a series of function calls which return some kind of Maybe value. This is something I often wish Rust supported, for instance when matching a type part of which is stored in a reference-counting pointer, a second match expression is required to extract data from the reference-counted part.

### Function serialization

Being a pure language, Orchid carries the potential to serialize functions and send them over the network. This enables for instance an Orchid web framework to represent UI as well as and database transactions as simple callbacks in server code that are flush with the code describing server behaviour. I would like to explore this option in the future and develop a general library that allows 

### Macros for UI, declarative testing, etc. 

The flexible macro system enables library developers to invent their own syntax for essentially anything. I considered defining macros for html, music scores / midi data, marble and flow diagrams.

### Unsafe

These functions may be exposed by a direct Orchid interpreter but they would probably not be included in the library exposed by an embedder.

#### system calls

While individual system APIs can be exposed to the program using dedicated Rust bindings, this takes time and limits the power of the language. The general solution to this in high level languages is to expose the `system()` function which enables high level code to interact with _some kind of shell_, the shell of the operating system. What shell this exactly is and what tools are available through it is up to the user to discover.

#### DMA/MMIO

As a high level language, Orchid doesn't inherently have direct memory access, in part because it's not generally required. Regardless, a way of writing to and reading from exact memory addresses may be useful in the development of libraries that interface with hardware such as a rapsberry pi's GPIO pins.

On general this is probably better accomplished using Rust functions that interface with Orchid, but this will eventually inevitably lead to several functions that do nothing but read a number from an address or write a number to an address, except the addresses are wrapped in various tagged structs. This repetition could be nipped in the bud by simply exposing a function for mmio and allowing the Orchid side to define the wrappers.

## Type system

### Early plans

Originally, Orchid was meant to have a type system that used Orchid itself to build generic types using logic of unconstrained complexity from their arguments. The time constraints did not allow for this to be done in the initial version, but it is still on the roadmap.

### Alternatives

During initial testing of the working version, I found that the most common kind of programming error in lambda calculus appears to be arity mismatch or syntax error that results in arity mismatch. Without any kind of type checking this is especially difficult to debug as every function looks the same. This can be addressed with a much simpler type system similar to System-F. Any such type checker would have to be constructed so as to only verify user-provided information regarding the arity of functions without attempting to find the arity of every expression, since System-F is strongly normalising and Orchid like any general purpose language supports potentially infinite loops.

