# Parsing

Orchid expressions are similar in nature to lambda calculus or haskell, except whitespace is mostly irrelevant.  

## Names

`name` and `ns_name` tokens appear all over the place in this spec. They represent operators, function names, arguments, modules. A `name` is

1. the universally recognized operators `,`, `.`, `..` and `...` (comma and single, double and triple dot)
2. any C identifier
3. any sequence of name-safe characters starting with a character that cannot begin a C identifier. A name-safe character is any non-whitespace Unicode character other than

    - digits
    - the namespace separator `:`,
    - the parametric expression starters `\` and `@`,
    - the string and char delimiters `"` and `'`,
    - the various brackets`(`, `)`, `[`, `]`, `{` and `}`,
    - `,`, `.` and `$`

    This means that, in absence of a known list of names, `!importatn!` is a single name but `importatn!` is two names, as a name that starts as a C identifier cannot contain special characters. It also means that using non-English characters in Orchid variables is a really bad idea. This is intentional, identifiers that need to be repeated verbatim should only contain characters that appear on all latin keyboards.

There are also reserved words that cannot be used as names; `export` and `import`.

A `ns_name` is a sequence of one or more `name` tokens separated by the namespace separator `::`.

All tokens that do not contain `::` in the code may be `name` or `ns_name` depending on their context.

## Clauses

Clauses are the building blocks of Orchid's syntax. They belong to one of a couple categories:

- S-expressions are a parenthesized sequence of space-delimited `clause`s. All three types of brackets `()`, `[]` and `{}` are supported.
- Lambdas start with `\<name>.`, followed by a sequence of `clause`s where `<name>` is a single `name` or `$_` followed by a C identifier. This is a greedy pattern that ends at the end of an enclosing S-expression, or the end of input.
- numbers can be in decimal, binary with the `0b` prefix, hexadecimal with the `0x` prefix, or octal with the `0` prefix. All bases support the decimal point, exponential notation or both. The exponent is prefixed with `p`, always written in decimal, may be negative, and it represents a power of the base rather than a power of 10. For example, `0xf0.4p-2` is `0xf04 / 16 ^ 3` or ~0.9385.
- Strings are delimited with `"`, support `\` escapes and four digit unicode escapes of the form `\uXXXX`. They may contain line breaks.
- Chars are a single character or escape from the above description of a string delimited by `'`.
- Placeholders are either of three styles; `$name`, `..$name`, `...$name`, `..$name:p`, `...$name:p`. the name is always a C identifier, p is an integer growth priority.
- Names are a single `ns_name`

## Files

Files are separated into lines. A line is delimited by newlines and only contains newlines within brackets. A line may be an import, rule, exported rule, or explicit export.

### Rules

Rules have the following form

```
pattern =priority=> template
```

The pattern is able to define new operators implicitly by referencing them, so all tokens must be delimited by spaces. The template is inserted in place of the pattern without parentheses, so unless it's meant to be part of a pattern matched by another rule which expects a particular parenthesization, when more than one token is produced the output should be wrapped in parentheses.

A shorthand syntax is available for functions:

```
name := value
```

name in this case must be a single `name`. Value is automatically parenthesized, and the priority of these rules is always zero.

### Explicit exports and exported rules

An explicit export consists of `export :: ( <names> )` where `<names>` is a comma-separated list of `name`s.

An exported rule consists of the keyword `export` followed by a regular rule. It both counts as a rule and an export of all the `name`s within the pattern.

### Imports

An import is a line starting with the keyword `import`, followed by a tree of imported names.

```
import_tree = name
            | name :: import_tree
            | name :: *
            | ( import_tree [, import_tree]+ )
```

Some examples of valid imports:

```
import std::cpsio
import std::(conv::parse_float, cpsio, str::*)
import std
```

Some examples of invalid imports:

```
import std::()
import std::cpsio::(print, *)
import std::(cpsio)
```

> **info**
> 
> while none of these are guaranteed to work currently, there's little reason they would have to be invalid, so future specifications may allow them.

An import can be normalized into a list of independent imports ending either with a `*` called wildcard imports or with a `name`. wildcard imports are normalized to imports for all the `name`s exported from the parent module. All Name clauses in the file starting with the same `name` one of these imports ended with are prefixed with the full import path. The rest of the Name clauses are prefixed with the full path of the current module.

Reference cycles in Orchid modules are never allowed, so the dependency of a module's exports on its imports and a wildcard's import's value on the referenced module's exports does not introduce the risk of circular dependencies, it just specifies the order of processing for files.
