import super::known::*

--[ Do nothing. Especially useful as a passive cps operation ]--
export identity := \x.x
--[
  Apply the function to the given value. Can be used to assign a
  concrete value in a cps assignment statement.
]--
export pass := \val.\cont. cont val
--[
  Apply the function to the given pair of values. Mainly useful to assign
  a concrete pair of values in a cps multi-assignment statement
]--
export pass2 := \a.\b.\cont. cont a b
--[
  A function that returns the given value for any input. Also useful as a
  "break" statement in a "do" block.
]--
export const := \a. \b.a

export ...$prefix $ ...$suffix:1 =0x1p38=> ...$prefix (...$suffix)
export ...$prefix |> $fn ..$suffix:1 =0x2p32=> $fn (...$prefix) ..$suffix

export ($name) => ...$body =0x2p129=> (\$name. ...$body)
export ($name, ...$argv) => ...$body =0x2p129=> (\$name. (...$argv) => ...$body)
$name => ...$body =0x1p129=> (\$name. ...$body)