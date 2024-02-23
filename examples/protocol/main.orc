protocol animal (
  const noise := vcall "noise"
)

type dog (
  const new := \name. wrap name
  impl super::animal := map::new [
    "noise" = \dog. "${dog}: Woof!"
  ]
)

type cat (
  const new := wrap 0
  impl super::animal := map::new [
    "noise" = \_. "a cat: Mew!"
  ]
)

const main := do {
  list::new [dog::new "Pavlov", cat::new]
    |> list::map (\a. println $ animal::noise a)
    |> list::chain exit_status::success
}
