mod common;
mod decode;
mod encode;
mod hierarchy;

#[macro_use]
extern crate quote;
#[macro_use]
extern crate syn;

#[allow(unused)]
use orchid_api_traits::Coding;
use proc_macro::TokenStream;

#[proc_macro_derive(Decode)]
pub fn decode(input: TokenStream) -> TokenStream { decode::derive(input) }

#[proc_macro_derive(Encode)]
pub fn encode(input: TokenStream) -> TokenStream { encode::derive(input) }

#[proc_macro_derive(Hierarchy, attributes(extends, extendable))]
pub fn hierarchy(input: TokenStream) -> TokenStream { hierarchy::derive(input) }

#[proc_macro_derive(Coding)]
pub fn coding(input: TokenStream) -> TokenStream {
	decode(input.clone()).into_iter().chain(encode(input)).collect()
}
