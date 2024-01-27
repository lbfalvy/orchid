import super::(option, tuple, tuple::t, panic, pmatch, pmatch::=>, macro, tee)
import super::(functional::*, procedural::*)
import super::(loop::*, bool::*, known::*, number::*)

as_type list ()

export const cons := \hd. \tl. wrap (option::some t[hd, unwrap tl])
export const end := wrap option::none
export const pop := \list. \default. \f. (
  pmatch::match (unwrap list) {
    option::none => default;
    option::some t[hd, tl] => f hd (wrap tl);
  }
)

-- Operators

--[ Fold each element into an accumulator using an `acc -> el -> acc`. #eager ]--
export const fold := \list. \acc. \f. (
  loop_over (list, acc) {
    cps head, list = pop list acc;
    let acc = f acc head;
  }
)

--[ Fold each element into an accumulator in reverse order. #eager-notail ]--
export const rfold := \list. \acc. \f. (
  recursive r (list)
    pop list acc \head. \tail.
      f (r tail) head
)

--[ Reverse a list. #eager ]--
export const reverse := \list. fold list end \tl. \hd. cons hd tl

--[ Fold each element into a shared element with an `el -> el -> el`. #eager-notail ]--
export const reduce := \list. \f. do{
  cps head, list = pop list option::none;
  option::some $ fold list head f
}

--[
  Return a new list that contains only the elements from the input list
  for which the function returns true. #lazy
]--
export const filter := \list. \f. (
  pop list end \head. \tail.
    if (f head)
    then cons head (filter tail f)
    else filter tail f
)

--[ Transform each element of the list with an `el -> any`. #lazy ]--
export const map := \list. \f. (
  recursive r (list)
    pop list end \head. \tail.
      cons (f head) (r tail)
)

--[ Skip `n` elements from the list and return the tail. #lazy ]--
export const skip := \foo. \n. (
  loop_over (foo, n) {
    cps _head, foo = if n <= 0
      then return foo
      else pop foo end;
    let n = n - 1;
  }
)

--[ Return `n` elements from the list and discard the rest. #lazy ]--
export const take := \list. \n. (
  recursive r (list, n)
    if n == 0
    then end
    else pop list end \head. \tail.
      cons head $ r tail $ n - 1
)

--[ Return the `n`th element from the list. #eager ]--
export const get := \list. \n. (
  loop_over (list, n) {
    cps head, list = pop list option::none;
    cps if n == 0
      then return (option::some head)
      else identity;
    let n = n - 1;
  }
)

--[ Map every element to a pair of the index and the original element. #lazy ]--
export const enumerate := \list. (
  recursive r (list, n = 0) 
    pop list end \head. \tail.
      cons t[n, head] $ r tail $ n + 1
)

--[
  Turn a list of CPS commands into a sequence. This is achieved by calling every
  element on the return value of the next element with the tail passed to it.
  The continuation is passed to the very last argument. #lazy
]--
export const chain := \list. \cont. loop_over (list) {
  cps head, list = pop list cont;
  cps head;
}

macro new[..$items] =0x2p84=> mk_list macro::comma_list (..$items)

macro mk_list ( macro::list_item $item $tail ) =0x1p254=> (cons $item mk_list $tail)
macro mk_list macro::list_end =0x1p254=> end

export ::(new)

( macro pmatch::request (cons $head $tail)
  =0x1p230=> await_subpatterns
    (pmatch::request ($head))
    (pmatch::request ($tail))
)
( macro await_subpatterns
    (pmatch::response $h_expr ( $h_binds ))
    (pmatch::response $t_expr ( $t_binds ))
  =0x1p230=> pmatch::response (
    pop
      pmatch::value
      pmatch::fail
      \head. \tail. (
        (\pmatch::pass. (\pmatch::value. $h_expr) head)
        (pmatch::take_binds $h_binds (
          (\pmatch::pass. (\pmatch::value. $t_expr) tail)
          (pmatch::take_binds $t_binds (
            pmatch::give_binds pmatch::chain_binds $h_binds $t_binds pmatch::pass
          ))
        ))
      )
  )
  (pmatch::chain_binds $h_binds $t_binds)
)
