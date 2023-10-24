import super::known::*
import super::match::*
import super::macro
import super::match::(match, =>)

--[ Do nothing. Especially useful as a passive cps operation ]--
export const identity := \x.x
--[
  Apply the function to the given value. Can be used to assign a
  concrete value in a cps assignment statement.
]--
export const pass := \val. \cont. cont val
--[
  Apply the function to the given pair of values. Mainly useful to assign
  a concrete pair of values in a cps multi-assignment statement
]--
export const pass2 := \a. \b. \cont. cont a b
--[
  A function that returns the given value for any input. Also useful as a
  "break" statement in a "do" block.
]--
export const return := \a. \b.a

export macro ...$prefix $ ...$suffix:1 =0x1p38=> ...$prefix (...$suffix)
export macro ...$prefix |> $fn ..$suffix:1 =0x2p32=> $fn (...$prefix) ..$suffix

( macro (..$argv) => ...$body
  =0x2p127=> lambda_walker macro::comma_list (..$argv) (...$body)
)
( macro $_arg => ...$body
  =0x2p127=> \$_arg. ...$body)
( macro lambda_walker ( macro::list_item ($_argname) $tail ) $body
  =0x2p254=> \$_argname. lambda_walker $tail $body
)
( macro lambda_walker ( macro::list_item (...$head) $tail ) $body
  =0x1p254=> \arg. match arg {
    ...$head => lambda_walker $tail $body;
  }
)
( macro lambda_walker macro::list_end $body
  =0x1p254=> $body
)
