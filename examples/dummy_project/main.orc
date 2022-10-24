opaque := \T. T

--[ Typeclass definition (also just a type) ]--
define Add $L:type $R:type $O:type as $L -> $R -> $O
-- HKTC
define Mappable $C:(type -> type) as @T. @U. (T -> U) -> $C T -> $C U
-- Dependency on existing typeclass
define Zippable $C:(type -> type) as @:Mappable $C. (
    @T. @U. @V. (T -> U -> V) -> $C T -> $C U -> $C V
)
define Default $T:type as $T
-- Is the intersection of typeclasses an operation we need?

--[ Type definition ]--
define Cons $elem:type as loop \r. Option (Pair T $elem)
nil := @T. from @(Cons T) none
cons := @T. \el:T. (
    generalise @(Cons T) 
    |> (\list. some t[el, into list]) 
    |> categorise @(Cons T)
)
export map := @T. @U. \f:T -> U. (
    generalise @(Cons T)
    |> loop ( \recurse. \option.
        map option \pair. t[f (fst pair), recurse (snd pair)] 
    )
    |> categorise @(Cons U) 
)
-- Universal typeclass implementation; no parameters, no overrides, no name for overriding
impl Mappable Cons via map
-- Blanket typeclass implementation; parametric, may override, must have name for overriding
impl (@T. Add (Cons T) (Cons T) (Cons T)) by concatenation over elementwiseAddition via concat

-- Scratchpad

filterBadWords := @C:type -> type. @:Mappable C. \strings:C String. (
    map strings \s. if intersects badWords (slice " " s) then none else some s
):(C (Option String))

-- /Scratchpad

main := \x. foo @bar x

foo := @util. \x. util x

export opaque := \T. atom