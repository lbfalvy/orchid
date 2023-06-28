import super::(option, fn::*, proc::*, loop::*, bool::*, known::*, num::*)

pair := \a.\b. \f. f a b

-- Constructors

export cons := \hd.\tl. option::some (pair hd tl)
export end := option::none

export pop := \list.\default.\f.list default \cons.cons f

-- Operators

--[
  Fold each element into an accumulator using an `acc -> el -> acc`.
  This evaluates the entire list, and is always tail recursive.
]--
export fold := \list.\acc.\f. (
  loop_over (list, acc) {
    cps head, list = pop list acc;
    let acc = f acc head;
  }
)

--[
  Fold each element into an accumulator in reverse order.
  This evaulates the entire list, and is never tail recursive.
]--
export rfold := \list.\acc.\f. (
  recursive r (list)
    pop list acc \head.\tail.
      f (r tail) head
)

--[
  Fold each element into a shared element with an `el -> el -> el`.
  This evaluates the entire list, and is never tail recursive.
]--
export reduce := \list.\f. do{
  cps head, list = pop list option::none;
  option::some $ fold list head f
}

--[
  Return a new list that contains only the elements from the input list
  for which the function returns true. This operation is lazy.
]--
export filter := \list.\f. (
  pop list end \head.\tail.
    if (f el)
    then cons el (filter tail f)
    else filter tail f
)

--[
  Transform each element of the list with an `el -> any`.
]--
export map := \list.\f. (
  recursive r (list)
    pop list end \head.\tail.
      cons (f head) (r tail)
)

--[
  Skip `n` elements from the list and return the tail
  If `n` is not an integer, this returns `end`.
]--
export skip := \list.\n. (
  loop_over (list, n) {
    cps _head, list = if n == 0
      then const list
      else pop list end;
    let n = n - 1;
  }
)

--[
  Return `n` elements from the list and discard the rest.
  This operation is lazy.
]--
export take := \list.\n. (
  recursive r (list, n)
    if n == 0
    then end
    else pop list end \head.\tail.
      cons head $ r tail $ n - 1
)

--[
  Return the `n`th element from the list.
  This operation is tail recursive.
]--
export get := \list.\n. (
  loop_over (list, n) {
    cps head, list = pop list option::none;
    cps if n == 0
      then const (option::some head)
      else identity;
    let n = n - 1;
  }
)

new[...$item, ...$rest:1] =0x2p84=> (cons (...$item) new[...$rest])
new[...$end] =0x1p84=> (cons (...$end) end)
new[] =0x1p84=> end

export ::(new)
