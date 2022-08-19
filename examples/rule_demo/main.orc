export main := [foo, bar, baz, quz]

[...$data] := (cons_start ...$data cons_carriage(none))

[] := none

, $item cons_carriage($tail) := cons_carriage(
  (some (cons $item $tail))
)

cons_start $item cons_carriage($tail) := some (cons $item $tail)

