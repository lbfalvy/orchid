import super::(known::*, bool::*, number::*, match, macro)

export type ty (
  import super::super::(number::*, bool::*, macro, panic)

  const discard_args := \n. \value. (
    if n == 0 then value
    else \_. discard_args (n - 1) value
  )

  macro gen_call macro::list_end =0x1p254=> \f.f
  macro gen_call ( macro::list_item $item $tail ) =0x1p254=> \f. (gen_call $tail) (f $item)
  export macro new ( $list ) =0x1p84=> wrap \f. (gen_call $list) (f (macro::length $list))

  export const pick := \tuple. \i. (unwrap tuple) ( \size.
    if size <= i then panic "Tuple index out of bounds"
    else discard_args i \val. discard_args (size - 1 - i) val
  )

  export const length := \tuple. (unwrap tuple) \size. discard_args size size

  export const apply := \tuple. \f. (unwrap tuple) f
)

const pick := ty::pick
const length := ty::length
const apply := ty::apply

macro t[..$items] =0x2p84=> ( ty::new ( macro::comma_list (..$items) ) )

export ::(t, size)

macro size ( t[..$items] ) =0x1p230=> macro::length macro::comma_list (..$items)

--[
  request l -> pattern_walker l
  pattern_walker end -> pattern_result
  pattern_walker h ++ t -> await_pattern
  await_pattern -> pattern_result
]--

( macro match::request ( t[ ..$items ] )
  =0x1p230=> tuple_pattern
    ( macro::length macro::comma_list ( ..$items ) )
    (
      pattern_walker
        (0) -- index of next item
        macro::comma_list ( ..$items ) -- leftover items
    )
)
( macro tuple_pattern $length ( pattern_result $expr ( $binds ) )
  =0x1p254=> match::response (
    if length match::value == $length
      then $expr
      else match::fail
  ) ( $binds )
)
( macro pattern_walker $length macro::list_end
  =0x1p254=> pattern_result match::pass ( match::no_binds )
)
( macro pattern_walker (...$length) ( macro::list_item $next $tail )
  =0x1p254=> pattern_await
    (...$length)
    ( match::request $next )
    ( pattern_walker (...$length + 1) $tail )
)
( macro pattern_await $length
    ( match::response $expr ( $binds ) )
    ( pattern_result $tail_expr ( $tail_binds ) )
  =0x1p254=>
    pattern_result
      (
        (\match::pass. (\match::value. $expr) (pick match::value $length)) (
          match::take_binds $binds (
            (\match::pass. $tail_expr) ( match::take_binds $tail_binds (
              match::give_binds
                match::chain_binds $binds $tail_binds
                match::pass
            ))
          )
        )
      )
      ( match::chain_binds $binds $tail_binds )
)
