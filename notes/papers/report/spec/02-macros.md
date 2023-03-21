# Macros

After parsing, what remains is a set of macro rules, each with a pattern, priority and template. Modules aren't tracked in this stage, their purpose was to namespace the tokens within the rules.

By employing custom import logic, it's also possible to add rules bypassing the parser. Starting with the macro phase, `clause`s may also be `atom`s or `externfn`s. The role of these is detailed in the [[03-runtime]] section.

Macros are executed in reverse priority order, each macro is checked against each subsection of each clause sequence. When a match is found, the substitution is performed and all macros are executed again.

## Placeholders

Patterns fall into two categories

- scalar placeholders
  - `$name` matches exactly one clause
  - `$_name` matches exactly one Name clause
- vectorial placeholders
  - `..$name` matches zero or more clauses
  - `...$name` matches one or more clauses

`$_name` is uniquely valid in the position of an argument name within a lambda.

Vectorial placeholders may also have a positive decimal integer growth priority specified after the name, separated with a `:` like so: `...$cond:2`. If it isn't specified, the growth priority defaults to 0.

The template may only include placeholders referenced in the pattern. All occurences of a placeholder within a rule must match the same things.

## Execution

Each clause in the pattern matches clauses as follows:

-  Name matches name with the same full path.
-  Lambda matches a lambda with matching argument name and matching body. If the argument name in the pattern is a name-placeholder (as in `\$_phname.`), the argument name in the source is treated as a module-local Name clause.
-  Parenthesized expressions match each other if the contained sequences match and both use the same kind of parentheses.
-  Placeholders' matched sets are as listed in [Placeholders].

If a pattern contains the same placeholder name more than once, matches where they don't match perfectly identical clauses, names or clause sequences are discarded.

### Order of preference

The growth order of vectorial placeholders is 

- Outside before inside parentheses
- descending growth priority
- left-to-right by occurrence in the pattern.

If a pattern matches a sequence in more than one way, whichever match allocates more clauses to the first vectorial placeholder in growth order is preferred.
