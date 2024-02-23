import std::conv::to_string

export const main := do{
  let foo = list::new[1, 2, 3, 4, 5, 6];
  let bar = list::map foo n => n * 2;
  let sum = bar
    |> list::skip 2
    |> list::take 3
    |> list::reduce ((a, b) => a + b)
    |> option::assume;
  cps println $ to_string sum;
  exit_status::success
}
