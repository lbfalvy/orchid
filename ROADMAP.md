This document is a wishlist, its items aren't ordered in any way other than inline notes about dependency relations

# Language

## Operator declarations
A dedicated (exportable) line type for declaring operators. Still just names, only you can write them next to other things without whitespace

- ops may not contain c-ident-safe characters
- clusters of operator characters are broken up with a greedy algorithm

## Typeclasses
Elixir-style protocols probably, only with n-ary dispatch which I saw in SICP-js

# Rules

## Placeholder constraints
Simultaneously match a pattern to a subexpression and give it a name to copy it over

- Copy unique 1->1 names over by default to preserve their location info

# STL

## Command short-circuiting
Functions for each command type which destructure it and pass it to an Orchid callback

## Runtime error handling
result? multipath cps utils? Not sure yet.

## Pattern matching
This was the main trick in Orchid, still want to do it, still need to polish the language first

## Macro error handling
Error tokens with rules to lift them out. Kinda depends on preservation of location info in rules to be really useful

# Systems

## Async
Join allows to run code when a tuple of pending events all resolve on the event poller

## New: FS
Exposes tree operations to Orchid
Uses existing IO to open and read files
Uses the event bus to read directories in batches without blocking other Orchid code

## New: Network
Event-driven I/O with single-fire events and resubscription to relay backpressure to the OS. Initially TCP

## New: Marshall
Serialization of Orchid data, including code, given customizable sets of serializable foreign items. Alternatively, code reflection so that all this can go in the STL
