const main := println "Hello World!" exit_status::success

macro (
	rule match ...$expr { ...$body } => '(
		fn::pass (...$expr) \match::value. ...$(
			fn::pass (quote::split body ';) \cases.
			fn::pass (list::map cases \x. (
				fn::pass (quote::split_once x '=>) \pair.
				tuple::destr pair 2 \req. \handler.
				fn::pass (macro::run '(match::request (...$key))) \match_res.
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
	Conceptually, all matches are compared.
		1. Within a macro, the top rule wins
		2. If two winning matches are not within the same macro,
			the one that matches the outermost, first token wins,
			including tokens that are immediately captured, such that a rule starting with .. is
			beaten by the same rule parenthesized and any rule starting with a scalar is beaten
			by the same rule prefixed with a vectorial
		3. If two winning matches start with the same token, an ambiguity error is raised

	
]--
