import super::(known::*, bool::*, number::*, string::*, fn::*)
import super::loop::recursive
import super::(pmatch, macro, panic, conv, list, option)

-- referenced in the impl table in Rust
const to_string_impl := \t. "tuple[" ++ (
  to_list t
    |> list::map conv::to_string
    |> list::reduce (\l. \r. l ++ ", " ++ r)
    |> option::fallback ""
) ++ "]"

export const to_list := \t. (
  recursive r (n=length t, l=list::end)
    if n == 0 then l
    else r (n - 1) (list::cons (pick t $ conv::to_uint $ n - 1) l)
)

macro gen_tuple $tup macro::list_end =0x1p254=> $tup
macro gen_tuple $tup ( macro::list_item $item $tail ) =0x1p254=> (gen_tuple (push $tup $item) $tail)
export macro new ( $list ) =0x1p84=> (gen_tuple empty $list)

macro t[..$items] =0x2p84=> ( new ( macro::comma_list (..$items) ) )

export ::(t, size)

--[
  request l -> tuple_pattern pattern_walker l
  pattern_walker end -> pattern_result
  pattern_walker h ++ t -> pattern_await ( request h ) ( pattern_walker t )
  pattern_await response pattern_result -> pattern_result
  tuple_pattern pattern_result -> response
]--

( macro pmatch::request ( t[ ..$items ] )
  =0x1p230=> tuple_pattern
    ( macro::length macro::comma_list ( ..$items ) )
    (
      pattern_walker
        macro::comma_list ( ..$items ) -- leftover items
    )
)
( macro tuple_pattern $length ( pattern_result $expr ( $binds ) )
  =0x1p254=> pmatch::response (
    if length pmatch::value == $length
      then ((\tuple_idx. $expr ) 0)
      else pmatch::fail
  ) ( $binds )
)
( macro pattern_walker macro::list_end
  =0x1p254=> pattern_result pmatch::pass ( pmatch::no_binds )
)
( macro pattern_walker ( macro::list_item $next $tail )
  =0x1p254=> pattern_await
    ( pmatch::request $next )
    ( pattern_walker $tail )
)
( macro pattern_await
    ( pmatch::response $expr ( $binds ) )
    ( pattern_result $tail_expr ( $tail_binds ) )
  =0x1p254=>
    pattern_result
      (
        (\pmatch::pass. (\pmatch::value. $expr) (pick pmatch::value tuple_idx)) (
          pmatch::take_binds $binds (
            (\pmatch::pass. (\tuple_idx. $tail_expr) (tuple_idx + 1))
            ( pmatch::take_binds $tail_binds (
              pmatch::give_binds
                (pmatch::chain_binds $binds $tail_binds)
                pmatch::pass
            ))
          )
        )
      )
      ( ( pmatch::chain_binds $binds $tail_binds ) )
)
