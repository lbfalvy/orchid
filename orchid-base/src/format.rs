use std::cmp::Ordering;
use std::convert::Infallible;
use std::future::Future;
use std::iter;
use std::rc::Rc;
use std::str::FromStr;

use itertools::Itertools;
use never::Never;
use regex::Regex;

use crate::interner::Interner;
use crate::{api, match_mapping};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct FmtUnit {
	pub subs: Vec<FmtUnit>,
	pub variants: Rc<Variants>,
}
impl FmtUnit {
	pub fn new(variants: Rc<Variants>, subs: impl IntoIterator<Item = FmtUnit>) -> Self {
		Self { subs: subs.into_iter().collect(), variants }
	}
	pub fn from_api(api: &api::FormattingUnit) -> Self {
		Self {
			subs: api.subs.iter().map(Self::from_api).collect(),
			variants: Rc::new(Variants(
				(api.variants.iter().map(|var| Variant {
					bounded: var.bounded,
					elements: var.elements.iter().map(FmtElement::from_api).collect(),
				}))
				.collect(),
			)),
		}
	}
	pub fn to_api(&self) -> api::FormattingUnit {
		api::FormattingUnit {
			subs: self.subs.iter().map(Self::to_api).collect(),
			variants: (self.variants.0.iter().map(|var| api::FormattingVariant {
				bounded: var.bounded,
				elements: var.elements.iter().map(FmtElement::to_api).collect(),
			}))
			.collect(),
		}
	}
	pub fn sequence(
		delim: &str,
		seq_bnd: Option<bool>,
		seq: impl IntoIterator<Item = FmtUnit>,
	) -> Self {
		let items = seq.into_iter().collect_vec();
		FmtUnit::new(Variants::sequence(items.len(), delim, seq_bnd), items)
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
		Ok(Self { subs: vec![], variants: Rc::new(Variants::default().bounded(s)) })
	}
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum FmtElement {
	Sub { slot: u32, bounded: Option<bool> },
	String(Rc<String>),
	Indent(Vec<FmtElement>),
}
impl FmtElement {
	pub fn str(s: &'_ str) -> Self { Self::String(Rc::new(s.to_string())) }
	pub fn sub(slot: u32, bounded: Option<bool>) -> Self { Self::Sub { slot, bounded } }
	pub fn bounded(i: u32) -> Self { Self::sub(i, Some(true)) }
	pub fn unbounded(i: u32) -> Self { Self::sub(i, Some(false)) }
	pub fn last(i: u32) -> Self { Self::sub(i, None) }
	pub fn sequence(len: usize, bounded: Option<bool>) -> impl Iterator<Item = Self> {
		let len32: u32 = len.try_into().unwrap();
		(0..len32 - 1).map(FmtElement::unbounded).chain([FmtElement::sub(len32 - 1, bounded)])
	}
	pub fn from_api(api: &api::FormattingElement) -> Self {
		match_mapping!(api, api::FormattingElement => FmtElement {
			Indent(v => v.iter().map(FmtElement::from_api).collect()),
			String(s => Rc::new(s.clone())),
			Sub{ *slot, *bounded },
		})
	}
	pub fn to_api(&self) -> api::FormattingElement {
		match_mapping!(self, FmtElement => api::FormattingElement {
			Indent(v => v.iter().map(FmtElement::to_api).collect()),
			String(s => s.to_string()),
			Sub{ *slot, *bounded },
		})
	}
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Variant {
	pub bounded: bool,
	pub elements: Vec<FmtElement>,
}

#[test]
fn variants_parse_test() {
	let vars = Variants::default().bounded("({0})");
	println!("final: {vars:?}")
}

#[derive(Clone, Debug, Hash, PartialEq, Eq, Default)]
pub struct Variants(pub Vec<Variant>);
impl Variants {
	fn parse_phs(s: &'_ str) -> Vec<FmtElement> {
		let re = Regex::new(r"(?<tpl>\{\d+?[bl]?\})|(\{\{)|(\}\})").unwrap();
		let matches = re.captures_iter(s);
		let slots = matches.into_iter().filter_map(|m| m.name("tpl")).map(|tpl| {
			let no_opencurly = tpl.as_str().strip_prefix("{").expect("required by regex");
			let maybe_dash = no_opencurly.strip_suffix("}").expect("required by regex");
			// we know it's not empty
			let last_char = maybe_dash.as_bytes()[maybe_dash.len() - 1] as char;
			let (num, bounded) = if !last_char.is_ascii_digit() {
				let bounded = match last_char {
					'b' => Some(true),
					'l' => None,
					_ => panic!("Invalid modifier char"),
				};
				(&maybe_dash[0..maybe_dash.len() - 1], bounded)
			} else {
				(maybe_dash, Some(false))
			};
			let idx = num.parse::<u32>().expect("Decimal digits required by regex");
			(tpl.range(), idx, bounded)
		});
		(iter::once(None).chain(slots.into_iter().map(Some)).chain([None]).tuple_windows())
			.flat_map(|(l, r)| {
				let string = match (l, &r) {
					(None, Some((r, ..))) => &s[..r.start],
					(Some((r1, ..)), Some((r2, ..))) => &s[r1.end..r2.start],
					(Some((r, ..)), None) => &s[r.end..],
					(None, None) => s,
				};
				let str_item = FmtElement::String(Rc::new(string.replace("{{", "{").replace("}}", "}")));
				match r {
					None => itertools::Either::Left([str_item]),
					Some((_, idx, bounded)) =>
						itertools::Either::Right([str_item, FmtElement::Sub { slot: idx, bounded }]),
				}
				.into_iter()
			})
			.coalesce(|left, right| match (left, right) {
				(FmtElement::String(left), FmtElement::String(right)) =>
					Ok(FmtElement::String(Rc::new(left.to_string() + right.as_str()))),
				tuple => Err(tuple),
			})
			.collect_vec()
	}
	fn parse(s: &'_ str) -> Vec<FmtElement> {
		let mut lines = s.lines();
		let Some(mut cur) = lines.next() else { return vec![] };
		return indent_blk(&mut cur, &mut lines, 0);
		fn indent_blk<'a>(
			cur: &mut &'a str,
			lines: &mut impl Iterator<Item = &'a str>,
			blk_lv: usize,
		) -> Vec<FmtElement> {
			let mut out = Vec::new();
			loop {
				let line_lv = cur.chars().take_while(|c| *c == '\t').count();
				match line_lv.cmp(&blk_lv) {
					Ordering::Greater => out.push(FmtElement::Indent(indent_blk(cur, lines, blk_lv + 1))),
					Ordering::Equal => out.extend(Variants::parse_phs(&cur[blk_lv..])),
					Ordering::Less => return out,
				}
				match lines.next() {
					Some(line) => *cur = line,
					None => return out,
				}
			}
		}
	}
	fn add(&mut self, bounded: bool, s: &'_ str) {
		self.0.push(Variant { bounded, elements: Self::parse(s) })
	}
	// This option is available in all positions
	pub fn bounded(mut self, s: &'_ str) -> Self {
		self.add(true, s);
		self
	}
	// This option is only available in positions immediately preceding the end of
	// the sequence or a parenthesized subsequence.
	pub fn unbounded(mut self, s: &'_ str) -> Self {
		self.add(false, s);
		self
	}
	pub fn sequence(len: usize, delim: &str, seq_bnd: Option<bool>) -> Rc<Self> {
		let seq = Itertools::intersperse(FmtElement::sequence(len, seq_bnd), FmtElement::str(delim));
		Rc::new(Variants(vec![Variant { bounded: true, elements: seq.collect_vec() }]))
	}
	pub fn units(self: &Rc<Self>, subs: impl IntoIterator<Item = FmtUnit>) -> FmtUnit {
		FmtUnit::new(self.clone(), subs)
	}
}
impl From<Rc<String>> for Variants {
	fn from(value: Rc<String>) -> Self {
		Self(vec![Variant { elements: vec![FmtElement::String(value)], bounded: true }])
	}
}
impl From<String> for Variants {
	fn from(value: String) -> Self { Self::from(Rc::new(value)) }
}
impl FromStr for Variants {
	type Err = Infallible;
	fn from_str(s: &str) -> Result<Self, Self::Err> { Ok(Self::default().bounded(s)) }
}

fn indent_str(s: &str, indent: u16) -> String {
	s.replace("\n", &format!("\n{}", "\t".repeat(indent.into())))
}

fn fill_slots<'a, 'b>(
	elements: impl IntoIterator<Item = &'a FmtElement>,
	values: &[FmtUnit],
	indent: u16,
	last_bounded: bool,
) -> String {
	elements
		.into_iter()
		.map(|el| match el {
			FmtElement::String(s) => indent_str(s, indent),
			FmtElement::Sub { slot, bounded } =>
				indent_str(&take_first(&values[*slot as usize], bounded.unwrap_or(last_bounded)), indent),
			FmtElement::Indent(elements) => fill_slots(elements, values, indent + 1, last_bounded),
		})
		.collect()
}

/// The simplest possible print strategy
pub fn take_first(unit: &FmtUnit, bounded: bool) -> String {
	let first = unit.variants.0.iter().find(|v| v.bounded || bounded).expect("No bounded variant!");
	fill_slots(&first.elements, &unit.subs, 0, bounded)
}

pub async fn take_first_fmt(v: &(impl Format + ?Sized), i: &Interner) -> String {
	take_first(&v.print(&FmtCtxImpl { i }).await, false)
}

pub struct FmtCtxImpl<'a> {
	pub i: &'a Interner,
}

pub trait FmtCtx {
	fn i(&self) -> &Interner;
	// fn print_as(&self, p: &(impl Format + ?Sized)) -> impl Future<Output =
	// String> where Self: Sized {
	// 	async {
	// 		// for now, always take the first option which is probably the one-line
	// form 		let variants = p.print(self).await;
	// 		take_first(&variants, true)
	// 	}
	// }
}
impl FmtCtx for FmtCtxImpl<'_> {
	fn i(&self) -> &Interner { self.i }
}

pub trait Format {
	fn print<'a>(&'a self, c: &'a (impl FmtCtx + ?Sized + 'a)) -> impl Future<Output = FmtUnit> + 'a;
}
impl Format for Never {
	async fn print<'a>(&'a self, _c: &'a (impl FmtCtx + ?Sized + 'a)) -> FmtUnit { match *self {} }
}
