Substitution rules are represented by the `=prio=>` arrow where `prio` is a floating point literal. They are tested form highest priority to lowest. When one matches, the substitution is executed and all macros are re-checked from the beginning.

Wildcards either match a single token `$foo`, at least one token `...$bar` or any number of tokens `..$baz`. The latter two forms can also have an unsigned integer growth priority `...$quz:3` which influences their order in deciding the precedence of matches.

# Match priority

When a macro matches the program more than once, matches in ancestors take precedence. If there's no direct ancestry, the left branch takes precedence. When two matches are found in the same token sequence, the order is determined by the number of tokens allocated to the highest priority variable length wildcard where this number differs.

Variable length placeholders outside parens always have a higher priority than those inside. On the same level, the numbers decide the priority. In case of a tie, the placeholder to the left is preferred.

# Writing macros

Macro programs are systems consisting of substitution rules which reinterpret the tree produced by the previous rules. A good example for how this works can be found in ../examples/list-processing/fn.orc

Priority numbers are written in hexadecimal normal form to avoid precision bugs, and they're divided into bands throughout the f64 value range: (the numbers represent powers of 16)

- **32-39**: Binary operators, in inverse priority order
- **80-87**: Expression-like structures such as if/then/else
- **128-135**: Anything that creates lambdas
  Programs triggered by a lower priority pattern than this can assume that all names are correctly bound
- **200**: Aliases extracted for readability
  The user-accessible entry points of all macro programs must be lower priority than this, so any arbitrary syntax can be extracted into an alias with no side effects
- **224-231**: Integration; documented hooks exposed by a macro package to allow third party packages to extend its functionality
  The `statement` pattern produced by `do{}` blocks and matched by `let` and `cps` is a good example of this. When any of these are triggered, all macro programs are in a documented state.
- **248-255**: Transitional states within macro programs get the highest priority

The numbers are arbitrary and up for debate. These are just the ones I came up with when writing the examples.