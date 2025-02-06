use orchid_api_derive::Coding;

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub struct FormattingUnit {
	pub subs: Vec<FormattingUnit>,
	pub variants: Vec<FormattingVariant>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub struct FormattingVariant {
	pub bounded: bool,
	pub elements: Vec<FormattingElement>,
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Coding)]
pub enum FormattingElement {
	Sub { slot: u32, bounded: Option<bool> },
	String(String),
	Indent(Vec<FormattingElement>),
}
