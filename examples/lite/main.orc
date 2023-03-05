TRUE := \t.\f.t
FALSE := \t.\f.f
NOT := \x.x FALSE TRUE
AND := \x.\y.x y FALSE
OR := \x.\y. x TRUE y
Y := \f.(\x.f (x x))(\x.f (x x))

(! ...$expr) =10=> (NOT ...$expr)
(...$lhs & ...$rhs) =10=> (AND (...$lhs) (...$rhs))
(...$lhs | ...$rhs) =20=> (OR (...$lhs) (...$rhs))

main := (TRUE & TRUE | FALSE & FALSE)

(start_token ...$rest) ==> (carriage(()) ...$rest)
(..$prefix carriage($state) $next ..$rest) ==> (..$prefix $out carriage(??) ..$rest)
