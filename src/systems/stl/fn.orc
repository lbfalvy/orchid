import super::known::*

--[ Do nothing. Especially useful as a passive cps operation ]--
export const identity := \x.x
--[
  Apply the function to the given value. Can be used to assign a
  concrete value in a cps assignment statement.
]--
export const pass := \val.\cont. cont val
--[
  Apply the function to the given pair of values. Mainly useful to assign
  a concrete pair of values in a cps multi-assignment statement
]--
export const pass2 := \a.\b.\cont. cont a b
--[
  A function that returns the given value for any input. Also useful as a
  "break" statement in a "do" block.
]--
export const return := \a. \b.a

export macro ...$prefix $ ...$suffix:1 =0x1p38=> ...$prefix (...$suffix)
export macro ...$prefix |> $fn ..$suffix:1 =0x2p32=> $fn (...$prefix) ..$suffix

export macro ($name) => ...$body =0x2p129=> (\$name. ...$body)
export macro ($name, ...$argv) => ...$body =0x2p129=> (\$name. (...$argv) => ...$body)
macro $name => ...$body =0x1p129=> (\$name. ...$body)
