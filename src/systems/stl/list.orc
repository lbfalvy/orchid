import super::option
import super::(functional::*, procedural::*, loop::*, bool::*, known::*, number::*, tuple::*)

const pair := \a. \b. \f. f a b

-- Constructors

export const cons := \hd. \tl. option::some t[hd, tl]
export const end := option::none

export const pop := \list. \default. \f. do{
  cps tuple = list default;
  cps head, tail = tuple;
  f head tail
}

-- Operators

--[
  Fold each element into an accumulator using an `acc -> el -> acc`.
  This evaluates the entire list, and is always tail recursive.
]--
export const fold := \list. \acc. \f. (
  loop_over (list, acc) {
    cps head, list = pop list acc;
    let acc = f acc head;
  }
)

--[
  Fold each element into an accumulator in reverse order.
  This evaulates the entire list, and is never tail recursive.
]--
export const rfold := \list. \acc. \f. (
  recursive r (list)
    pop list acc \head. \tail.
      f (r tail) head
)

--[
  Fold each element into a shared element with an `el -> el -> el`.
  This evaluates the entire list, and is never tail recursive.
]--
export const reduce := \list. \f. do{
  cps head, list = pop list option::none;
  option::some $ fold list head f
}

--[
  Return a new list that contains only the elements from the input list
  for which the function returns true. This operation is lazy.
]--
export const filter := \list. \f. (
  pop list end \head. \tail.
    if (f head)
    then cons head (filter tail f)
    else filter tail f
)

--[
  Transform each element of the list with an `el -> any`.
]--
export const map := \list. \f. (
  recursive r (list)
    pop list end \head. \tail.
      cons (f head) (r tail)
)

--[
  Skip `n` elements from the list and return the tail
  If `n` is not an integer, this returns `end`.
]--
export const skip := \foo. \n. (
  loop_over (foo, n) {
    cps _head, foo = if n == 0
      then return foo
      else pop foo end;
    let n = n - 1;
  }
)

--[
  Return `n` elements from the list and discard the rest.
  This operation is lazy.
]--
export const take := \list. \n. (
  recursive r (list, n)
    if n == 0
    then end
    else pop list end \head. \tail.
      cons head $ r tail $ n - 1
)

--[
  Return the `n`th element from the list.
  This operation is tail recursive.
]--
export const get := \list. \n. (
  loop_over (list, n) {
    cps head, list = pop list option::none;
    cps if n == 0
      then return (option::some head)
      else identity;
    let n = n - 1;
  }
)

--[
  Map every element to a pair of the index and the original element
]--
export const enumerate := \list. (
  recursive r (list, n = 0) 
    pop list end \head. \tail.
      cons t[n, head] $ r tail $ n + 1
)

--[
  Turn a list of CPS commands into a sequence. This is achieved by calling every
  element on the return value of the next element with the tail passed to it.
  The continuation is passed to the very last argument.
]--
export const chain := \list. \cont. loop_over (list) {
  cps head, list = pop list cont;
  cps head;
}

macro new[...$item, ...$rest:1] =0x2p84=> (cons (...$item) new[...$rest])
macro new[...$end] =0x1p84=> (cons (...$end) end)
macro new[] =0x1p84=> end

export ::(new)
