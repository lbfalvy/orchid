export ::(main, foo)

main := [foo, bar, baz, quz]

foo := steamed hams

[...$data] := (cons_start ...$data cons_carriage(none))

[] := none

...$prefix:1 , ...$item cons_carriage(
  $tail
) := ...$prefix cons_carriage(
  (some (cons (...$item) $tail))
)

cons_start ...$item cons_carriage($tail) := some (cons (...$item) $tail)

