import std::(map, option, fn::*)

export const vcall := \proto. \key. \val. (
  resolve proto val
    |> map::get key
    |> option::assume
    $ break val
)
