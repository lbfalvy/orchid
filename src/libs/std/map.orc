import super::(bool::*, functional::*, known::*, loop::*, procedural::*, string::*)
import super::(panic, pmatch, macro, option, list, tuple, to_string, conv, pmatch::[=>])

as_type map (
  impl to_string := \map. "map[" ++ (
    unwrap map
      |> list::map (
        (tuple::t[k, v]) => conv::to_string k ++ " = " ++ conv::to_string v
      )
      |> list::reduce (\l. \r. l ++ ", " ++ r)
      |> option::fallback ""
  ) ++ "]"
)

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

export ::having
( macro pmatch::request (having [..$items])
  =0x1p230=> having_pattern (
    pattern_walker
      macro::comma_list ( ..$items )
  )
)
( macro having_pattern ( tail_result $expr ( $binds ) )
  =0x1p254=> pmatch::response $expr ( $binds )
)
( macro pattern_walker macro::list_end
  =0x1p254=> tail_result pmatch::pass ( pmatch::no_binds )
)
( macro pattern_walker ( macro::list_item ( ...$key = ...$value:1 ) $tail )
  =0x1p254=> await_pattern ( ...$key )
    ( pmatch::request (...$value) )
    ( pattern_walker $tail )
)
( macro await_pattern $key
    ( pmatch::response $expr ( $binds ) )
    ( tail_result $t_expr ( $t_binds ) )
  =0x1p254=> tail_result (
    option::handle (get pmatch::value $key)
      pmatch::fail
      \value. (\pmatch::pass. (\pmatch::value. $expr) value) (
        pmatch::take_binds $binds (
          (\pmatch::pass. $t_expr) (
            pmatch::take_binds $t_binds (
              pmatch::give_binds pmatch::chain_binds $binds $t_binds pmatch::pass
            )
          )
        )
      )
  )
  ( pmatch::chain_binds $binds $t_binds )
)
