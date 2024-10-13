# Code sample for the new macro system

```
macro (
	rule match ...$expr { ...$body } => \recurse. '(
		fn::pass (...$expr) \match::value. ...$(
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

--[
	Macros are run from the top down.
	For every token
		1. If it's a name token, test all macros starting with that name
		2. If none match and this is the first token in a list, test all macros starting with vectorials
	Test all in a set of macros
		1. Take the first rule that matches in each block
		2. If there are multiple matches across blocks, raise an ambiguity error
		3. If the single match is in the recursion stack, raise a recursion error
		4. Add the matching rule to the recursion stack, then execute the body.
]--

--[
	1. Macro patterns are held in the host, they don't contain atoms, and atoms are never considered equal, so the matcher doesn't have to call an extension.
	2. The body of macros may be defined in Rust. If it isn't, the entire interpreter will recurse on the macro to calculate the output.
]--

--[
	1. if the rule body uses the same macro, fail with the rule
	2. if the rule explicitly recursively invokes the same macro, fail with the first match
]--