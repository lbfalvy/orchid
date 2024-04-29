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
      fn decode<R: std::io::Read>(read: &mut R) -> Self { #decode }
    }
  };
  TokenStream::from(expanded)
}

fn decode_fields(fields: &syn::Fields) -> pm2::TokenStream {
  match fields {
    syn::Fields::Unit => pm2::TokenStream::new(),
    syn::Fields::Named(_) => {
      let names = fields.iter().map(|f| f.ident.as_ref().unwrap());
      quote! { { #( #names: orchid_api_traits::Decode::decode(read), )* } }
    },
    syn::Fields::Unnamed(_) => {
      let exprs = fields.iter().map(|_| quote! { orchid_api_traits::Decode::decode(read), });
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
        match <u8 as orchid_api_traits::Decode>::decode(read) {
          #(#opts)*
          x => panic!("Unrecognized enum kind {x}")
        }
      }
    },
  }
}
