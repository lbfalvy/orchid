import list
import option
import fn::*
import std::to_string
import std::debug

-- utilities for using lists as pairs

export fst := \l. (
  list::get l 0
    (panic "nonempty expected")
    \x.x
)
export snd := \l. (
  list::get l 1
    (panic "2 elements expected")
    \x.x
)

-- constructors

export empty := list::end
export add := \m.\k.\v. (
  list::cons
    list::new[k, v]
    m
)

-- queries

-- return the last occurrence of a key if exists
export get := \m.\k. (
  loop r on (m) with
    list::pop m option::none \head.\tail. 
      if fst head == k
      then option::some $ snd head
      else r tail
)

-- commands

-- remove one occurrence of a key
export del := \m.\k. (
  loop r on (m) with
    list::pop m list::end \head.\tail.
      if fst head == k then tail
      else list::cons head $ r tail
)

-- remove all occurrences of a key
export delall := \m.\k. (
  loop r on (m) with
    list::pop m list::end \head.\tail.
      if (fst head) == k then r tail
      else list::cons head $ r tail
)

-- replace at most one occurrence of a key
export set := \m.\k.\v. (
  m
  |> del k
  |> add k v
)

-- ensure that there's only one instance of each key in the map
export normalize := \m. do{
  let normal = empty
  loop r on (m normal) with
    list::pop m normal \head.\tail.
      r tail $ set normal (fst head) (snd head)
}

new[...$tail:2, ...$key = ...$value:1] =0x2p84=> (
  set new[...$tail] (...$key) (...$value)
)
new[...$key = ...$value:1] =0x1p84=> (add empty (...$key) (...$value))
new[] =0x1p84=> empty

export ::(new)