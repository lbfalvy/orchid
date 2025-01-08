use std::iter;

use itertools::Itertools;
use pm2::TokenTree;
use proc_macro::TokenStream;
use proc_macro2 as pm2;
use syn::DeriveInput;

pub fn derive(input: TokenStream) -> TokenStream {
	// Parse the input tokens into a syntax tree
	let input = parse_macro_input!(input as syn::DeriveInput);
	let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
	let name = &input.ident;
	let extendable = is_extendable(&input);
	let is_leaf_val = if extendable { quote!(TLFalse) } else { quote!(TLTrue) };
	match get_ancestry(&input) {
		None => TokenStream::from(quote! {
			impl #impl_generics orchid_api_traits::InHierarchy for #name #ty_generics #where_clause {
				type IsRoot = orchid_api_traits::TLTrue;
				type IsLeaf = orchid_api_traits:: #is_leaf_val ;
			}
		}),
		Some(ancestry) => {
			let parent = ancestry[0].clone();
			let casts = gen_casts(&ancestry[..], &quote!(#name));
			TokenStream::from(quote! {
				#casts
				impl #impl_generics orchid_api_traits::InHierarchy for #name #ty_generics #where_clause {
					type IsRoot = orchid_api_traits::TLFalse;
					type IsLeaf = orchid_api_traits:: #is_leaf_val ;
				}
				impl #impl_generics orchid_api_traits::Extends for #name #ty_generics #where_clause {
					type Parent = #parent;
				}
			})
		},
	}
}

fn gen_casts(ancestry: &[pm2::TokenStream], this: &pm2::TokenStream) -> pm2::TokenStream {
	let from_impls = iter::once(this).chain(ancestry.iter()).tuple_windows().map(|(prev, cur)| {
		quote! {
			impl From<#this> for #cur {
				fn from(value: #this) -> Self {
					#cur::#prev(value.into())
				}
			}
		}
	});
	let try_from_impls = (1..=ancestry.len()).map(|len| {
		let (orig, inter) = ancestry[..len].split_last().unwrap();
		fn gen_chk(r: &[pm2::TokenStream], last: &pm2::TokenStream) -> pm2::TokenStream {
			match r.split_last() {
				None => quote! { #last (_) => true },
				Some((ty, tail)) => {
					let sub = gen_chk(tail, last);
					quote! {
						#ty ( value ) => match value {
							#ty:: #sub ,
							_ => false
						}
					}
				},
			}
		}
		let chk = gen_chk(inter, this);
		fn gen_unpk(r: &[pm2::TokenStream], last: &pm2::TokenStream) -> pm2::TokenStream {
			match r.split_last() {
				None => quote! { #last ( value ) => value },
				Some((ty, tail)) => {
					let sub = gen_unpk(tail, last);
					quote! {
						#ty ( value ) => match value {
							#ty:: #sub ,
							_ => unreachable!("Checked above!"),
						}
					}
				},
			}
		}
		let unpk = gen_unpk(inter, this);
		quote! {
			impl TryFrom<#orig> for #this {
				type Error = #orig;
				fn try_from(value: #orig) -> Result<Self, Self::Error> {
					let can_cast = match &value {
						#orig:: #chk ,
						_ => false
					};
					if !can_cast { return Err(value) }
					Ok ( match value {
						#orig:: #unpk ,
						_ => unreachable!("Checked above!")
					} )
				}
			}
		}
	});
	from_impls.chain(try_from_impls).flatten().collect()
}

fn get_ancestry(input: &DeriveInput) -> Option<Vec<pm2::TokenStream>> {
	input.attrs.iter().find(|a| a.path().get_ident().is_some_and(|i| *i == "extends")).map(|attr| {
		match &attr.meta {
			syn::Meta::List(list) => (list.tokens.clone().into_iter())
				.batching(|it| {
					let grp: pm2::TokenStream =
						it.take_while(|t| {
							if let TokenTree::Punct(punct) = t { punct.as_char() != ',' } else { true }
						})
						.collect();
					(!grp.is_empty()).then_some(grp)
				})
				.collect(),
			_ => panic!("The correct format of the parent macro is #[parent(SomeParentType)]"),
		}
	})
}

fn is_extendable(input: &DeriveInput) -> bool {
	input.attrs.iter().any(|a| a.path().get_ident().is_some_and(|i| *i == "extendable"))
}

#[test]
fn test_wtf() { eprintln!("{}", gen_casts(&[quote!(ExtHostReq)], &quote!(BogusReq))) }
