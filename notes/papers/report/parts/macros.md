# Macros

The macros describe several independent sequential programs that are expected to be able to interact with each other. To make debugging easier, the order of execution of independent macros should also be relatively static.

## Execution order

The macro executor follows a manually specified priority cascade, with priorities ranging from 0 to f64 max (0x2p1023, exclusive). Priorities are accepted in any valid floating point format, but usually written in binary or hexadecimal natural form, as this format represents floating point precision on the syntax level, thus making precision errors extremely unlikely.

The range of valid priorities is divided up into bands, much like radio bands. In this case, the bands serve to establish a high level ordering between instructions.

The bands are each an even 32 orders of magnitude, with space in between for future expansion

|               |          |             |              |
| :-----------: | :------: | :---------: | :----------: |
|     0-31      |  32-63   |    64-95    |    96-127    |
| optimizations |    x     |             |              |
|    128-159    | 160-191  |   192-223   |   224-255    |
|   operators   |          |             |      x       |
|    256-287    | 288-319  |   320-351   |   352-383    |
|               |          | expressions |              |
|    384-415    | 416-447  |   448-479   |   480-511    |
|               |    x     |             |              |
|    512-543    | 544-575  |   576-607   |   608-639    |
|   bindings    |          |             |      x       |
|    640-671    | 672-703  |   704-735   |   736-767    |
|               |          |      x      |              |
|    768-799    | 800-831  |   832-863   |   864-895    |
|               | aliases* |             |              |
|    896-927    | 928-959  |   960-991   |     992-     |
| integrations  |          |             | transitional |

### Transitional states

Transitional states produced and consumed by the same macro program occupy the range above 0x1p991. Nothing in this range should be written by the user or triggered by an interaction of distinct macro programs, the purpose of this high range is to prevent devices such as carriages from interacting. Any transformation sequence in this range can assume that the tree is inert other than its own operation.

### Integrations

Integrations expect an inert syntax tree but at least one token in the pattern is external to the macro program that resolves the rule, so it's critical that all macro programs be in a documented state at the time of resolution.

### Aliases

Fragments of code extracted for readability are all at exactly 0x1p800. These may be written by programmers who are not comfortable with macros or metaprogramming. They must have unique single token patterns. Because their priority is higher than any entry point, they can safely contain parts of other macro invocations. They have a single priority number because they can't conceivably require internal ordering adjustments and their usage is meant to be be as straightforward as possible.

### Binding builders

Syntax elements that manipulate bindings should be executed earlier. `do` blocks and (future) `match` statements are good examples of this category. Anything with a lower priority trigger can assume that all names are correctly bound.

### Expressions

Things that essentially work like function calls just with added structure, such as `if`/`then`/`else` or `loop`. These are usually just more intuitive custom forms that are otherwise identical to a macro

### Operators

Binary and unary operators that process the chunks of text on either side. Within the band, these macros are prioritized in inverse precedence order and apply to the entire range of clauses before and after themselves, to ensure that function calls have the highest perceived priority.

### Optimizations

Macros that operate on a fully resolved lambda code and look for known patterns that can be simplified. I did not manage to create a working example of this but for instance repeated string concatenation is a good example.