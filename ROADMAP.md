This document is a wishlist, its items aren't ordered in any way other than inline notes about dependency relations

# Language

None! Thanks to very aggressive modularization, changes to the core language are almost never needed to achieve specific goals

# Rules

## Placeholder constraints
Simultaneously match a pattern to a subexpression and give it a name to copy it over

## Role annotations
Some way for the rule repository to record the roles certain tokens took in patterns, and some way for the macros to attach semantic information to these roles, so that dev tooling can understand the purpose of each token

# STL

## Command short-circuiting
Functions for each command type which destructure it and pass it to an Orchid callback

## Runtime error handling
result? multipath cps utils? Not sure yet.

## Macro error handling
Error tokens with rules to lift them out.

# Systems

## Async
Join allows to run code when a tuple of pending events all resolve on the event poller

## New: Network
Event-driven I/O with single-fire events and resubscription to relay backpressure to the OS. Initially TCP

## New: Marshall
Serialization of Orchid data, including code, given customizable sets of serializable foreign items. Alternatively, code reflection so that all this can go in the STL

# Miscellaneous

## Language server
A very rudimentary language server to visually indicate what the macros do to your code

## Type checker
In the distant hopeful future, I'd like to support a type system
