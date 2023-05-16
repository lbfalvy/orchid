# Fn

This file contains a variety of utilities for functional programming

```
export Y := \f.(\x.f (x x))(\x.f (x x))

export loop $r on (...$parameters) with ...$tail =0x5p512=> Y (\$r.
  bind_names (...$parameters) (...$tail)
) ...$parameters

-- bind each of the names in the first argument as a parameter for the second argument
bind_names ($name ..$rest) $payload =0x2p1000=> \$name. bind_names (..$rest) $payload
bind_names () (...$payload) =0x1p1000=> ...$payload

export ...$prefix $ ...$suffix:1 =0x1p130=> ...$prefix (...$suffix)
export ...$prefix |> $fn ..$suffix:1 =0x2p130=> $fn (...$prefix) ..$suffix

export (...$argv) => ...$body =0x2p512=> (bind_names (...$argv) (...$body))
$name => ...$body =0x1p512=> (\$name. ...$body)
```

## bind_names

This is a utility macro for binding a list of names on an expression. It demonstrates how to extract reusable macro program fragments to simplify common tasks. This demonstrative version simply takes a sequence of name tokens without any separators or custom programming, but its functionality can be extended in the future to include eg. destructuring.

## arrow functions

The arrow `=>` operator here is used to define inline functions. It is very similar to the native `\x.` lambda, except that native lambdas use higher priority than any macro so they can't appear inside a `do{}` block as all of the subsequent lines would be consumed by them. It is parsed using the following rules:
```
export (...$argv) => ...$body =0x2p512=> (bind_names (...$argv) (...$body))
$name => ...$body =0x1p512=> (\$name. ...$body)
```

## pipelines

This is a concept borrowed from Elixir. The `|>` operator simply inserts the output of the previous expression to the first argument of the following function.
```
export ...$prefix |> $fn ..$suffix:1 =0x2p130=> $fn (...$prefix) ..$suffix
```

It is processed left-to-right, but leaves the suffix on the same level as the function and sinks the prefix, which means that long pipelines eventually become left associative despite the inverted processing order.

## right-associative function call operator

The `$` operator is analogous to its Haskell counterpart. It is right-associative and very low priority. Its purpose is to eliminate trailing parentheses.

## Loop expression

Recursion in lambda calculus is achieved using a fixpoint combinator. The classic version of this combinator described by Church is the [Y-combinator][hb_tlc], defined like so:
```
export Y := \f.(\x.f (x x))(\x.f (x x))
```

[hb_tlc]: ISBN-0444867481

Formalizing what this does is difficult, in plain words it calls `f` with an expression that is equivalent to its own return value, thus giving the parameter a convenient means to define its value in terms of different parameterizations of itself. The following snippet computes 2^12 to demonstrate how it would normally be called.
```
export main := Y (\r.\n.\s.
  if n == 0 then s
  else r (n - 1) (s * 2)
) 12 0
```

The purpose of the loop expression is to provide a more convenient syntax to define recursive structures, as direct calls to the Y-combinator are error prone. It is defined as follows:
```
export loop $r on (...$parameters) with ...$tail =0x5p512=> Y (\$r.
  bind_names (...$parameters) (...$tail)
) ...$parameters
```

The template allows the caller to give the point of recursion a name and enumerate the names that can change value between iterations of the loop. The point of recursion then has to be called with the same number of parameters.

It may be possible to construct a variant of this statement which allows only reassigning subsets of the mutable parameter list. It is definitely possible to construct a variant that allows declaring new names in place in the parameter list, although I did not have time to do so.