use proc_macro::TokenStream;
use proc_macro2 as pm2;

use crate::common::add_trait_bounds;

pub fn derive(input: TokenStream) -> TokenStream {
	// Parse the input tokens into a syntax tree
	let input = parse_macro_input!(input as syn::DeriveInput);
	let generics = add_trait_bounds(input.generics, parse_quote!(orchid_api_traits::Decode));
	let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
	let name = input.ident;
	let decode = decode_body(&input.data);
	let expanded = quote! {
		impl #impl_generics orchid_api_traits::Decode for #name #ty_generics #where_clause {
			async fn decode<R: orchid_api_traits::async_std::io::Read + ?Sized>(
				mut read: std::pin::Pin<&mut R>
			) -> Self {
				#decode
			}
		}
	};
	TokenStream::from(expanded)
}

fn decode_fields(fields: &syn::Fields) -> pm2::TokenStream {
	match fields {
		syn::Fields::Unit => quote! {},
		syn::Fields::Named(_) => {
			let exprs = fields.iter().map(|f| {
				let syn::Field { ty, ident, .. } = &f;
				quote! {
					#ident : (Box::pin(< #ty as orchid_api_traits::Decode>::decode(read.as_mut()))
						as std::pin::Pin<Box<dyn std::future::Future<Output = _>>>).await
				}
			});
			quote! { { #( #exprs, )* } }
		},
		syn::Fields::Unnamed(_) => {
			let exprs = fields.iter().map(|field| {
				let ty = &field.ty;
				quote! {
					(Box::pin(< #ty as orchid_api_traits::Decode>::decode(read.as_mut()))
						as std::pin::Pin<Box<dyn std::future::Future<Output = _>>>).await,
				}
			});
			quote! { ( #( #exprs )* ) }
		},
	}
}

fn decode_body(data: &syn::Data) -> proc_macro2::TokenStream {
	match data {
		syn::Data::Union(_) => panic!("Unions can't be deserialized"),
		syn::Data::Struct(str) => {
			let fields = decode_fields(&str.fields);
			quote! { Self #fields }
		},
		syn::Data::Enum(en) => {
			let opts = en.variants.iter().enumerate().map(|(i, v @ syn::Variant { ident, .. })| {
				let fields = decode_fields(&v.fields);
				let id = i as u8;
				quote! { #id => Self::#ident #fields, }
			});
			quote! {
				match <u8 as orchid_api_traits::Decode>::decode(read.as_mut()).await {
					#(#opts)*
					x => panic!("Unrecognized enum kind {x}")
				}
			}
		},
	}
}
