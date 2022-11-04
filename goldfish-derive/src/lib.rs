use proc_macro::TokenStream;

use quote::quote;
use syn::{parse::Parser, parse_macro_input, DeriveInput};

// #[proc_macro_attribute]
// pub fn manual_destruct(_args: TokenStream, input: TokenStream) -> TokenStream
// {
// 	let mut ast = parse_macro_input!(input as DeriveInput);

// 	match &mut ast.data
// 	{
// 		syn::Data::Struct(ref mut data) =>
// 		{
// 			match &mut data.fields
// 			{
// 				syn::Fields::Named(fields) =>
// 				{
// 					fields.named.push(
// 						syn::Field::parse_named
// 							.parse2(quote! {__destroyed_flag: bool})
// 							.unwrap(),
// 					);
// 				}
// 				_ => (),
// 			}
// 			quote! {
// 				#ast
// 			}
// 			.into()
// 		}
// 		_ => panic!("manual_destruct can only be used on structs!"),
// 	}
// }

// #[proc_macro_derive(ManualDestruct)]
// pub fn manual_destruct_macro(input: TokenStream) -> TokenStream
// {
// 	let ast = syn::parse(input).unwrap();

// 	impl_manual_destruct_macro(&ast)
// }

// fn impl_manual_destruct_macro(ast: &syn::DeriveInput) -> TokenStream
// {
// 	let identifier = &ast.ident;

// 	let gen = quote! {
// 		impl Drop for #identifier
// 		{
// 			fn drop(&mut self)
// 			{
//                 assert(!self.__destroyed_flag)
// 			}
// 		}
// 	};

// 	gen.into()
// }
