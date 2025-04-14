We make a distinction between positional and prioritized macros.

# Named macro

```
macro (
	rule match ...$expr { ...$body } => \recurse. '(
		fn::pass (...$expr) \match::value. ..$(
			fn::pass (quote::split body ';) \cases.
			fn::pass (list::map cases \x. (
				fn::pass (quote::split_once x '=>) \pair.
				tuple::destr pair 2 \req. \handler.
				fn::pass (recurse '(match::request (...$key))) \match_res.
				quote::match '(match::response $decoder (...$bindings)) match_res \match_res_match.
				fn::pass (option::expect match_res_match "Invalid pattern ${key}") \res_parts.
				fn::pass (map::get_unwrap res_parts "decoder") \decoder.
				fn::pass (map::get_unwrap res_parts "bindings") \bindings.
				fn::pass (quote::to_list bindings) \binding_names.
				fn::pass (list::rfold handler \tail. \name. '( \ $name . $tail )) \success.
				'( $decoder $success )
			)) \case_fns.
			list::append case_fns '( panic "No cases match" )
		)
	)
)
```

Named macro patterns must start with a name token. They are always evaluated first. If they don't end with a vectorial placeholder, macro evaluation continues after them so that they can become first arguments to infix operators.

# Prioritized macro

```
macro 3 (
	...$lhs + ...$rhs:1 => \recurse. '(add (..$(recurse lhs)) (..$(recurse rhs)))
)
```

Prioritised macro patterns must start and end with a vectorial placeholder. They represent infix operators.

# Algorithm

Macros are checked from the outermost block inwards.

1. For every name token, test all named macros starting with that name
   1. If the tail is implicit, continue iterating
2. Test all prioritized macros
   1. Take the first rule that matches in the highest prioritized block

Test all in a set of macros
1. Take the first rule that matches in each block
2. If there are multiple matches across blocks, raise an ambiguity error
3. If the single match is in the recursion stack, raise a recursion error
4. Add the matching rule to the recursion stack, then execute the body.

# Considerations

Maxims for the location of macros

1. Macro patterns are held in the host, they don't contain atoms, and atoms are never considered equal, so the matcher doesn't have to call an extension.
2. The body of macros may be defined in Rust. If it isn't, the entire interpreter will recurse on the macro to calculate the output.

On recursion, the following errors can be detected

1. if the rule body uses the same macro, fail with the rule
2. if the rule explicitly recursively invokes the same macro, fail with the first match

# Elements of the extension

Recursion has to happen through the interpreter itself, so the macro system is defined in terms of atoms just like an extension

- atom `MacTree` depicts a single token. `MacTree(tpl)` is also a `MacTree` but it can contain the independently unrepresentable templated slot node

- lexer `'` followed by any single token always generates `MacTree`. If it contains placeholders which are tokens prefixed with `$` or `..$`, it generates a call to `instantiate_tpl` with a prepared `MacTree(tpl)` as the first argument and the placeholder values after. `MacTree(tpl)` only exists as an internal subresult routed directly to `instantiate_tpl`.

- line parser `macro` parses a macro with the existing logic
- atom `MacRecurState` holds the recursion state
- function `resolve_recur` finds all matches on a MacTree
  - type: `MacRecurState -> MacTree -> MacTree`
  - use all relevant macros to find all matches in the tree
    - since macros must contain a locally defined token, it can be assumed that at the point that a constant is evaluated and all imports in the parent module have been resolved, necessarily all relevant macro rules must have been loaded
  - for each match
    - check for recursion violations
    - wrap the body in iife-s corresponding to the named values in the match state
    - emit a recursive call to process and run the body, and pass the same recursive call as argument for the macro to use
		```
		(\recur. lower (recur $body) recur)
			(resolve_recur $mac_recur_state)
		```
  - emit a single call to `instantiate_tpl` which receives all of these
- function `instantiate_tpl` inserts `MacTree` values into a `MacTree(tpl)`
  - type: `MacTree(tpl) [-> MacTree] -> MacTree`  
		_this function deduces the number of arguments from the first argument. This combines poorly with autocurry, but it's an easy way to avoid representing standalone tree lists_
  - walks the tree to find max template slot number, reads and type checks as many template values
  - returns the populated tree
- function `resolve` is the main entry point of the code
  - type: `MacTree -> MacTree`
  - invokes `resolve_recur` with an empty `MacRecurState`
- function `lower` is the main exit point of the code
  - type: `MacTree -> any`
  - Lowers `MacTree` into the equivalent `Expr`.