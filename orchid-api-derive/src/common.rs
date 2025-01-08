use proc_macro2 as pm2;
use quote::ToTokens;
use syn::spanned::Spanned;

pub fn add_trait_bounds(mut generics: syn::Generics, bound: syn::TypeParamBound) -> syn::Generics {
	for param in &mut generics.params {
		if let syn::GenericParam::Type(ref mut type_param) = *param {
			type_param.bounds.push(bound.clone())
		}
	}
	generics
}

pub fn destructure(fields: &syn::Fields) -> Option<pm2::TokenStream> {
	match fields {
		syn::Fields::Unit => None,
		syn::Fields::Named(_) => {
			let field_list = fields.iter().map(|f| f.ident.as_ref().unwrap());
			Some(quote! { { #(#field_list),* } })
		},
		syn::Fields::Unnamed(un) => {
			let field_list = (0..fields.len()).map(|i| pos_field_name(i, un.span()));
			Some(quote! { ( #(#field_list),* ) })
		},
	}
}

pub fn pos_field_name(i: usize, span: pm2::Span) -> pm2::TokenStream {
	syn::Ident::new(&format!("field_{i}"), span).to_token_stream()
}
