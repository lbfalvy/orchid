import super::(option, match, macro)
import super::(functional::*, procedural::*)
import super::(loop::*, bool::*, known::*, number::*, tuple::*)

export type ty (
  import super::super::(option, tuple, panic)
  import super::super::(known::*, bool::*)

  export const cons := \hd. \tl. wrap (option::some tuple::t[hd, unwrap tl])
  export const end := wrap option::none
  export const pop := \list. \default. \f. (
    option::handle (unwrap list)
      default
      \pair. tuple::apply pair
        \len. if len == 2
          then ( \hd. \tl. f hd (wrap tl) )
          else panic "list element must be 2-ple"
  )
)

export const cons := ty::cons
export const end := ty::end
export const pop := ty::pop

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

macro new[..$items] =0x2p84=> mk_list macro::comma_list (..$items)

macro mk_list ( macro::list_item $item $tail ) =0x1p254=> (cons $item mk_list $tail)
macro mk_list macro::list_end =0x1p254=> end

export ::(new)

( macro match::request (cons $head $tail)
  =0x1p230=> await_subpatterns
    (match::request ($head))
    (match::request ($tail))
)
( macro await_subpatterns
    (match::response $h_expr ( $h_binds ))
    (match::response $t_expr ( $t_binds ))
  =0x1p230=> match::response (
    pop
      match::value
      match::fail
      \head. \tail. (
        (\match::pass. (\match::value. $h_expr) head)
        (match::take_binds $h_binds (
          (\match::pass. (\match::value. $t_expr) tail)
          (match::take_binds $t_binds (
            match::give_binds match::chain_binds $h_binds $t_binds match::pass
          ))
        ))
      )
  )
  (match::chain_binds $h_binds $t_binds)
)
