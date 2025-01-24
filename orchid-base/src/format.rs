use std::convert::Infallible;
use std::iter;
use std::rc::Rc;
use std::str::FromStr;

use itertools::Itertools;
use regex::Regex;

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct FmtUnit {
	pub subs: Vec<FmtUnit>,
	pub variants: Rc<Variants>,
}
impl FmtUnit {
	pub fn new(variants: Rc<Variants>, subs: impl IntoIterator<Item = FmtUnit>) -> Self {
		Self { subs: subs.into_iter().collect(), variants }
	}
}
impl<T> From<T> for FmtUnit
where Variants: From<T>
{
	fn from(value: T) -> Self { Self { subs: vec![], variants: Rc::new(Variants::from(value)) } }
}
impl FromStr for FmtUnit {
	type Err = Infallible;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(Self { subs: vec![], variants: Rc::new(Variants::new([s])) })
	}
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum FmtElement {
	Sub(u8),
	String(Rc<String>),
	InlineSub(u8),
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Variants(pub Vec<Vec<FmtElement>>);
impl Variants {
	pub fn new<'a>(variants: impl IntoIterator<Item = &'a str>) -> Self {
		let re = Regex::new(r"(?<tpl>\{\d+?-?\})|(\{\{)|(\}\})").unwrap();
		Self(Vec::from_iter(variants.into_iter().map(|s: &str| {
			let matches = re.captures_iter(s);
			let slots = matches.into_iter().filter_map(|m| m.name("tpl")).map(|tpl| {
				let no_opencurly = tpl.as_str().strip_prefix("{").expect("required by regex");
				let maybe_dash = no_opencurly.strip_suffix("}").expect("required by regex");
				let (num, had_dash) =
					maybe_dash.strip_suffix('-').map_or((maybe_dash, false), |s| (s, true));
				let idx = num.parse::<u8>().expect("Decimal digits required by regex");
				(tpl.range(), idx, had_dash)
			});
			(iter::once(None).chain(slots.into_iter().map(Some)).chain(None).tuple_windows())
				.flat_map(|(l, r)| {
					let string = match (l, &r) {
						(None, Some((r, ..))) => &s[..r.start],
						(Some((r1, ..)), Some((r2, ..))) => &s[r1.end..r2.start],
						(Some((r, ..)), None) => &s[r.end..],
						(None, None) => s,
					};
					let str_item = FmtElement::String(Rc::new(string.to_string()));
					match r {
						None => itertools::Either::Left([str_item]),
						Some((_, idx, inline)) => itertools::Either::Right([str_item, match inline {
							true => FmtElement::InlineSub(idx),
							false => FmtElement::Sub(idx),
						}]),
					}
					.into_iter()
				})
				.coalesce(|left, right| match (left, right) {
					(FmtElement::String(left), FmtElement::String(right)) =>
						Ok(FmtElement::String(Rc::new(left.to_string() + right.as_str()))),
					tuple => Err(tuple),
				})
				.collect_vec()
		})))
	}
}
impl From<String> for Variants {
	fn from(value: String) -> Self { Self(vec![vec![FmtElement::String(Rc::new(value))]]) }
}
impl From<Rc<String>> for Variants {
	fn from(value: Rc<String>) -> Self { Self(vec![vec![FmtElement::String(value)]]) }
}
impl FromStr for Variants {
	type Err = Infallible;
	fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self::new([s])) }
}
