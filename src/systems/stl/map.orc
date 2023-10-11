import super::(bool::*, functional::*, known::*, list, option, loop::*, procedural::*)
import std::panic

-- utilities for using lists as pairs

export const fst := \l. (
  list::get l 0
    (panic "nonempty expected")
    \x.x
)
export const snd := \l. (
  list::get l 1
    (panic "2 elements expected")
    \x.x
)

-- constructors

export const empty := list::end
export const add := \m. \k. \v. (
  list::cons
    list::new[k, v]
    m
)

-- queries

-- return the last occurrence of a key if exists
export const get := \m. \key. (
  loop_over (m) {
    cps record, m = list::pop m option::none;
    cps if fst record == key
      then return $ option::some $ snd record
      else identity;
  }
)

-- commands

-- remove one occurrence of a key
export const del := \m. \k. (
  recursive r (m)
    list::pop m list::end \head. \tail.
      if fst head == k then tail
      else list::cons head $ r tail
)

-- remove all occurrences of a key
export const delall := \m. \k. (
  list::filter m \record. fst record != k
)

-- replace at most one occurrence of a key
export const set := \m. \k. \v. (
  m
  |> del k
  |> add k v
)

-- ensure that there's only one instance of each key in the map
export const normalize := \m. (
  recursive r (m, normal=empty)
    list::pop m normal \head. \tail.
      r tail $ set normal (fst head) (snd head)
)

macro new[...$tail:2, ...$key = ...$value:1] =0x2p84=> (
  set new[...$tail] (...$key) (...$value)
)
macro new[...$key = ...$value:1] =0x1p84=> (add empty (...$key) (...$value))
macro new[] =0x1p84=> empty

export ::(new)
