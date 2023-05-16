This example showcases common list processing functions and some functional programming utilities. It is also the first multi-file demo.

_in main.orc_
```
import std::(to_string, print)
import super::list
import fn::*

export main := do{
  let foo = list::new[1, 2, 3, 4, 5, 6];
  let bar = list::map foo n => n * 2;
  let sum = bar
    |> list::skip 2
    |> list::take 3
    |> list::reduce 0 (a b) => a + b;
  cps print $ to_string sum ++ "\n";
  0
}
```

This file imports `list` as a sibling module and `fn` as a top-level file. These files are in identical position, the purpose of this is just to test various ways to reference modules.

- The contents of _fn.orc_ are described in [fn](./fn.md)
- _list.orc_ and its dependency, _option.orc_ are described in [list](./list.md)

---

The `main` function uses a `do{}` block to enclose a series of name bindings. It constructs a list of numbers 1-6. This is done eagerly, or at least a linked list of the same size is constructed eagerly, although the `cons` calls are left until the first read. Due to Orchid's laziness, `bar` gets assigned the `map` call as-is. `sum` is assigned from the `|>` pipe chain, which is essentially the same as a chain of further name bindings; the return value of each function is passed as the first argument of the next, pushing subsequent arguments out of the way.

When the `print` expression is evaluated, the updates are applied as needed; the mapping is never applied to 1 and 2, and none of the loops in the list processing functions execute their body on the list object containing 6.