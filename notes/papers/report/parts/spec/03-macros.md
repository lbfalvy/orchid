# Macros

After parsing, what remains is a set of macro rules, each with a pattern, priority and template. Modules aren't tracked at this stage, their purpose was to namespace the tokens within the rules.

By employing custom import logic, it's also possible to add rules bypassing the parser. Starting with the macro phase, `clause`s may also be `atom`s or `externfn`s. The role of these is detailed in the [[04-runtime]] section.

Macros are tested in order of descending priority, each macro is checked against each subsection of each clause sequence. When a match is found, the substitution is performed and all macros are checked again.

## Placeholders

Patterns fall into two categories

- scalar placeholders
  - `$name` matches exactly one clause, including a parenthesized sequence.
- vectorial placeholders
  - `..$name` matches zero or more clauses
  - `...$name` matches one or more clauses

Vectorial placeholders may also have a positive decimal integer growth priority specified after the name, separated with a `:` like so: `...$cond:2`. If it isn't specified, the growth priority defaults to 0.

Any single clause can appear in the position of a lambda argument during macro execution. By the end of the macro execution phase, all lambdas must have a Name in the position of argument.

The template may only include placeholders referenced in the pattern. Two vectorial placeholders cannot appear next to each other in the pattern.\
A placeholder name can only appar once in a pattern.\

## Execution

Each clause in the pattern matches clauses as follows:

-  Name matches a Name with the same fully resolved namespaced name.
-  Lambda matches a Lambda with matching argument and matching body. Lambda arguments are module-local Name clauses, so if they are moved out of the body by a macro they can become unbound or refer to a previously shadowed global.
-  Parenthesized expressions match each other if the contained sequences match and both use the same delimiters.
-  Placeholders' matched sets are as listed in [Placeholders](#placeholders).

### Precedence of matches

The growth order of vectorial placeholders is 

- Outside before inside parentheses
- descending growth priority
- right-to-left by occurrence in the pattern.

If a pattern matches a sequence in more than one way, whichever match allocates more clauses to the highest vectorial placeholder in growth order is preferred.

Rules are conceptually extended with a vectorial placeholder of priority 0 on either end unless a vectorial placeholder is already present there. In practice, this means that multiple occurences of a scalar pattern within a sequence are matched left to right.
