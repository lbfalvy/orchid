import super::(bool::*, functional::*, known::*, loop::*, procedural::*)
import super::(panic, match, macro, option, list)

export type ty (
  import super::super::(panic, macro, list, tuple, option)
  import super::super::(bool::*, functional::*, known::*, loop::*, procedural::*)

  --[ Constructors ]--

  const empty := wrap list::end
  const add := \m. \k. \v. wrap (
    list::cons
      tuple::t[k, v]
      (unwrap m)
  )

  --[ List constructor ]--

  export ::new
  macro new[..$items] =0x2p84=> mk_map macro::comma_list (..$items)

  macro mk_map macro::list_end =0x1p254=> empty
  ( macro mk_map ( macro::list_item ( ...$key = ...$value:1 ) $tail )
    =0x1p254=> ( set mk_map $tail (...$key) (...$value) )
  )

  --[ Queries ]--
  
  -- return the last occurrence of a key if exists
  export const get := \m. \key. (
    loop_over (m=unwrap m) {
      cps record, m = list::pop m option::none;
      cps if tuple::pick record 0 == key
        then return $ option::some $ tuple::pick record 1
        else identity;
    }
  )

  --[ Commands ]--

  -- remove one occurrence of a key
  export const del := \m. \k. wrap (
    recursive r (m=unwrap m)
      list::pop m list::end \head. \tail.
        if tuple::pick head 0 == k then tail
        else list::cons head $ r tail
  )

  -- replace at most one occurrence of a key
  export const set := \m. \k. \v. m |> del k |> add k v
)

macro new =0x1p200=> ty::new

export const empty := ty::empty
export const add := ty::add
export const get := ty::get
export const set := ty::set
export const del := ty::del

export ::having
( macro match::request (having [..$items])
  =0x1p230=> having_pattern (
    pattern_walker
      macro::comma_list ( ..$items )
  )
)
( macro having_pattern ( tail_result $expr ( $binds ) )
  =0x1p254=> match::response $expr ( $binds )
)
( macro pattern_walker macro::list_end
  =0x1p254=> tail_result match::pass ( match::no_binds )
)
( macro pattern_walker ( macro::list_item ( ...$key = ...$value:1 ) $tail )
  =0x1p254=> await_pattern ( ...$key )
    ( match::request (...$value) )
    ( pattern_walker $tail )
)
( macro await_pattern $key
    ( match::response $expr ( $binds ) )
    ( tail_result $t_expr ( $t_binds ) )
  =0x1p254=> tail_result (
    option::handle (get match::value $key)
      match::fail
      \value. (\match::pass. (\match::value. $expr) value) (
        match::take_binds $binds (
          (\match::pass. $t_expr) (
            match::take_binds $t_binds (
              match::give_binds match::chain_binds $binds $t_binds match::pass
            )
          )
        )
      )
  )
  ( match::chain_binds $binds $t_binds )
)
