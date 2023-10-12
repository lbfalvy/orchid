import std::to_string

const fizz_buzz := n => (
  (recursive r (i=0) list::cons i $ r (i + 1))
    |> list::map (i =>
      if i % 15 == 0 then "FizzBuzz"
      else if i % 3 == 0 then "Fizz"
      else if i % 5 == 0 then "Buzz"
      else to_string i
    )
    |> list::take n
    |> list::reduce ((l, r) => l ++ "\n" ++ r)
    |> option::unwrap
)

const main := fizz_buzz 100
