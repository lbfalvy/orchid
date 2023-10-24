import std::known::(_, ;)
import std::procedural
import std::bool
import std::macro
import std::panic

--[
  The protocol:

  Request contains the pattern
  Response contains an expression and the list of names 
]--

(
  macro ..$prefix:1 match ...$argument:0 { ..$body } ..$suffix:1
  =0x1p130=> ..$prefix (
    (\value. match_walker macro::semi_list ( ..$body ) )
    ( ...$argument )
  ) ..$suffix
)

macro match_walker macro::list_end =0x1p254=> panic "no arms match"
( macro match_walker ( macro::list_item (...$pattern => ...$handler:1) $tail )
  =0x1p254=> match_await ( request (...$pattern) ) (...$handler) ( match_walker $tail )
)
( macro match_await ( response $expr ( $binds ) ) $handler $tail
  =0x1p254=> (\fail. (\pass. $expr) (take_binds $binds $handler)) $tail
)

macro request (( ..$pattern )) =0x1p254=> request ( ..$pattern )

-- bindings list

export ::(no_binds, add_bind, chain_binds, give_binds, take_binds)

macro add_bind $_new no_binds =0x1p254=> ( binds_list $_new no_binds )
( macro add_bind $_new ( binds_list ...$tail )
  =0x1p254=> ( binds_list $_new ( binds_list ...$tail ) )
)
macro give_binds no_binds $cont =0x1p254=> $cont
( macro give_binds ( binds_list $_name $tail ) $cont
  =0x1p254=> (give_binds $tail $cont $_name)
)
macro take_binds no_binds $cont =0x1p254=> $cont
( macro take_binds ( binds_list $_name $tail ) $cont
  =0x1p254=> \$_name. take_binds $tail $cont
)
macro chain_binds no_binds $second =0x1p254=> $second
( macro chain_binds ( binds_list $_head $tail ) $second
  =0x1p254=> add_bind $_head chain_binds $tail $second
)

--[ primitive pattern ( _ ) ]--

(
  macro request ( _ )
  =0x1p230=> response pass ( no_binds )
)

--[ primitive name pattern ]--

(
  macro request ( $_name )
  =0x1p226=> response ( pass value ) ( add_bind $_name no_binds )
)

--[ primitive pattern ( and ) ]--

( macro request ( ...$lhs bool::and ...$rhs )
  =0x3p230=> await_and_subpatterns ( request (...$lhs ) ) ( request ( ...$rhs ) )
)

( macro await_and_subpatterns ( response $lh_expr ( $lh_binds ) ) ( response $rh_expr ( $rh_binds ) )
  =0x1p254=> response (
      (\pass. $lh_expr) (take_binds $lh_binds (
        (\pass. $rh_expr) (take_binds $rh_binds (
          give_binds chain_binds $lh_binds $rh_binds pass
        ))
      ))
    )
    ( chain_binds $lh_binds $rh_binds ) 
)

--[ primitive pattern ( or ) ]--

(
  macro request ( ...$lhs bool::or ...$rhs )
  =0x3p230=> await_or_subpatterns
    ( request ( ...$lhs ) )
    ( request ( ...$rhs ) )
)

( -- for this to work, lh and rh must produce the same bindings
  macro await_or_subpatterns ( response $lh_expr ( $lh_binds) ) ( response $rh_expr ( $rh_binds ) )
  =0x1p254=> response (
    (\cancel. $lh_expr) -- lh works with pass directly because its bindings are reported up
    ($rh_expr (take_binds $rh_binds -- rh runs if lh cancels
      (give_binds $lh_binds pass) -- translate rh binds to lh binds
    ))
  )
  ( $lh_binds ) -- report lh bindings
)

export ::(match, cancel, argument, request, response, =>)
