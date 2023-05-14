This example showcases common list processing functions and some functional programming utilities. It is also the first multi-file demo.

_in main.orc_
```
import std::(to_string, print)
import super::list
import fn::*

export main := do{
  let foo = list::new[1, 2, 3, 4, 5, 6];
  let bar = list::map foo n => n * 2;
  let sum = bar
    |> list::skip 2
    |> list::take 3
    |> list::reduce 0 (a b) => a + b;
  cps print $ to_string sum ++ "\n";
  0
}
```

_in fn.orc_
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

_in list.orc_
```
import option
import super::fn::*

pair := \a.\b. \f. f a b

-- Constructors

export cons := \hd.\tl. option::some (pair hd tl)
export end := option::none

export pop := \list.\default.\f. list default \cons.cons f

-- Operators

export reduce := \list.\acc.\f. (
  loop r on (list acc) with
    pop list acc \head.\tail. r tail (f acc head)
)

export map := \list.\f. (
  loop r on (list) with
    pop list end \head.\tail. cons (f head) (r tail)
)

export skip := \list.\n. (
  loop r on (list n) with
    if n == 0 then list
    else pop list end \head.\tail. r tail (n - 1)
)

export take := \list.\n. (
  loop r on (list n) with
    if n == 0 then end
    else pop list end \head.\tail. cons head $ r tail $ n - 1
)

new[...$item, ...$rest:1] =0x2p333=> (cons (...$item) new[...$rest])
new[...$end] =0x1p333=> (cons (...$end) end)
new[] =0x1p333=> end

export ::(new)
```

_in option.orc_
```
export some := \v. \d.\f. f v
export none := \d.\f. d

export map := \option.\f. option none f
export flatten := \option. option none \opt. opt
export flatmap := \option.\f. option none \opt. map opt f
```

The `main` function uses a `do{}` block to enclose a series of name bindings. It imports `list` as a sibling module and `fn` as a top-level file. These files are in identical position, the purpose of this is just to test various ways to reference modules.

## fn

### bind_names

This is a utility macro for binding a list of names on an expression. It demonstrates how to extract reusable macro program fragments to simplify common tasks. This demonstrative version simply takes a sequence of name tokens without any separators or custom programming, but its functionality can be extended in the future to include eg. destructuring.

### arrow functions

The arrow `=>` operator here is used to define inline functions. It is very similar to the native `\x.` lambda, except that native lambdas use higher priority than any macro so they can't appear inside a `do{}` block as all of the subsequent lines would be consumed by them. It is parsed using the following rules:
```
export (...$argv) => ...$body =0x2p512=> (bind_names (...$argv) (...$body))
$name => ...$body =0x1p512=> (\$name. ...$body)
```

### pipelines

This is a concept borrowed from Elixir. The `|>` operator simply inserts the output of the previous expression to the first argument of the following function.
```
export ...$prefix |> $fn ..$suffix:1 =0x2p130=> $fn (...$prefix) ..$suffix
```

It is processed left-to-right, but leaves the suffix on the same level as the function and sinks the prefix, which means that long pipelines eventually become left associative despite the inverted processing order.

### right-associative function call operator

The `$` operator is analogous to its Haskell counterpart. It is right-associative and very low priority. Its purpose is to eliminate trailing parentheses.

### Loop expression

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