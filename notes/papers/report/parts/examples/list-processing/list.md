# List

These files demonstrate building datastructures using closures.

## Option.orc

Option is among the simplest datastructures. It either stores a value or nothing. To interact with it, one must provide a default value and a selector.

```
export some := \v. \d.\f. f v
export none := \d.\f. d

export map := \option.\f. option none f
export flatten := \option. option none \opt. opt
export flatmap := \option.\f. option none \opt. map opt f
```

The selector is required in lambda calculus because the only way to obtain information about values is to evaluate them, but it's not actually necessary in Orchid because it's always possible to pass a primitive of incompatible type as the default value and then use equality comparison to decide whether we got the value in the option or our dud. Regardless, this interface is vastly more convenient and probably more familiar to programmers coming from functional languages.

## List.orc

The linked list is an outstandingly powerful and versatile datastructure and the backbone of practical functional programming. This implementation uses a locally defined church pair and the option defined above in an effort to be more transparent, although this means that the essential operation of splitting the head and tail or returning a default value becomes an explicit function (here named `pop`) instead of the intrinsic interface of the list itself.

_in list.orc_
```
import option
import super::fn::*

pair := \a.\b. \f. f a b

-- Constructors

export cons := \hd.\tl. option::some (pair hd tl)
export end := option::none

-- Operators

export pop := \list.\default.\f. list default \cons.cons f

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

Most of these operations should be self-explanatory in the context of the parts defined in [fn.md](./fn.md).

The `new[]` macro builds a list from data. Because they are expected to contain expressions, the fields here are comma separated unlike in `fn::=>` and `fn::loop`. I did not find this inconsistency jarring during initial testing, but it may be updated if further improvements to `loop` and `=>`'s syntax open up the possibility of multi-token field descriptions.