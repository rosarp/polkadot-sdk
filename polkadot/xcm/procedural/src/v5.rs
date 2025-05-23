// Copyright (C) Parity Technologies (UK) Ltd.
// This file is part of Polkadot.

// Polkadot is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Polkadot is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Polkadot.  If not, see <http://www.gnu.org/licenses/>.

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{Result, Token};

const MAX_JUNCTIONS: usize = 8;

pub mod location {
	use super::*;

	/// Generates conversion functions from other types to the `Location` type:
	/// - [PalletInstance(50), GeneralIndex(1984)].into()
	/// - (Parent, Parachain(1000), AccountId32 { .. }).into()
	pub fn generate_conversion_functions(input: proc_macro::TokenStream) -> Result<TokenStream> {
		if !input.is_empty() {
			return Err(syn::Error::new(Span::call_site(), "No arguments expected"))
		}

		let from_tuples = generate_conversion_from_tuples(8, 8);

		Ok(quote! {
			#from_tuples
		})
	}

	fn generate_conversion_from_tuples(max_junctions: usize, max_parents: usize) -> TokenStream {
		let mut from_tuples = (0..=max_junctions)
			.map(|num_junctions| {
				let types = (0..num_junctions).map(|i| format_ident!("J{}", i)).collect::<Vec<_>>();
				let idents =
					(0..num_junctions).map(|i| format_ident!("j{}", i)).collect::<Vec<_>>();
				let array_size = num_junctions;
				let interior = if num_junctions == 0 {
					quote!(Junctions::Here)
				} else {
					let variant = format_ident!("X{}", num_junctions);
					quote! {
						Junctions::#variant( alloc::sync::Arc::new( [#(#idents .into()),*] ) )
					}
				};

				let mut from_tuple = quote! {
					impl< #(#types : Into<Junction>,)* > From<( Ancestor, #( #types ),* )> for Location {
						fn from( ( Ancestor(parents), #(#idents),* ): ( Ancestor, #( #types ),* ) ) -> Self {
							Location { parents, interior: #interior }
						}
					}

					impl From<[Junction; #array_size]> for Location {
						fn from(j: [Junction; #array_size]) -> Self {
							let [#(#idents),*] = j;
							Location { parents: 0, interior: #interior }
						}
					}
				};

				let from_parent_tuples = (0..=max_parents).map(|cur_parents| {
					let parents =
						(0..cur_parents).map(|_| format_ident!("Parent")).collect::<Vec<_>>();
					let underscores =
						(0..cur_parents).map(|_| Token![_](Span::call_site())).collect::<Vec<_>>();

					quote! {
						impl< #(#types : Into<Junction>,)* > From<( #( #parents , )* #( #types , )* )> for Location {
							fn from( ( #(#underscores,)* #(#idents,)* ): ( #(#parents,)* #(#types,)* ) ) -> Self {
								Self { parents: #cur_parents as u8, interior: #interior }
							}
						}
					}
				});

				from_tuple.extend(from_parent_tuples);
				from_tuple
			})
			.collect::<TokenStream>();

		let from_parent_junctions_tuples = (0..=max_parents).map(|cur_parents| {
			let parents = (0..cur_parents).map(|_| format_ident!("Parent")).collect::<Vec<_>>();
			let underscores =
				(0..cur_parents).map(|_| Token![_](Span::call_site())).collect::<Vec<_>>();

			quote! {
				impl From<( #(#parents,)* Junctions )> for Location {
					fn from( (#(#underscores,)* junctions): ( #(#parents,)* Junctions ) ) -> Self {
						Location { parents: #cur_parents as u8, interior: junctions }
					}
				}
			}
		});
		from_tuples.extend(from_parent_junctions_tuples);

		quote! {
			impl From<(Ancestor, Junctions)> for Location {
				fn from((Ancestor(parents), interior): (Ancestor, Junctions)) -> Self {
					Location { parents, interior }
				}
			}

			impl From<Junction> for Location {
				fn from(x: Junction) -> Self {
					Location { parents: 0, interior: [x].into() }
				}
			}

			#from_tuples
		}
	}
}

pub mod junctions {
	use super::*;

	pub fn generate_conversion_functions(input: proc_macro::TokenStream) -> Result<TokenStream> {
		if !input.is_empty() {
			return Err(syn::Error::new(Span::call_site(), "No arguments expected"))
		}

		// Support up to 8 Parents in a tuple, assuming that most use cases don't go past 8 parents.
		let from_v4 = generate_conversion_from_v4(MAX_JUNCTIONS);
		let from_tuples = generate_conversion_from_tuples(MAX_JUNCTIONS);

		Ok(quote! {
			#from_v4
			#from_tuples
		})
	}

	fn generate_conversion_from_tuples(max_junctions: usize) -> TokenStream {
		(1..=max_junctions)
			.map(|num_junctions| {
				let idents =
					(0..num_junctions).map(|i| format_ident!("j{}", i)).collect::<Vec<_>>();
				let types = (0..num_junctions).map(|i| format_ident!("J{}", i)).collect::<Vec<_>>();

				quote! {
					impl<#(#types : Into<Junction>,)*> From<( #(#types,)* )> for Junctions {
						fn from( ( #(#idents,)* ): ( #(#types,)* ) ) -> Self {
							[#(#idents .into()),*].into()
						}
					}
				}
			})
			.collect()
	}

	fn generate_conversion_from_v4(max_junctions: usize) -> TokenStream {
		let match_variants = (0..max_junctions)
			.map(|cur_num| {
				let num_ancestors = cur_num + 1;
				let variant = format_ident!("X{}", num_ancestors);
				let idents = (0..=cur_num).map(|i| format_ident!("j{}", i)).collect::<Vec<_>>();
				let convert = idents
					.iter()
					.enumerate()
					.map(|(index, ident)| {
						quote! { let #ident = core::convert::TryInto::try_into(slice[#index].clone())?; }
					})
					.collect::<Vec<_>>();

				quote! {
					crate::v4::Junctions::#variant( arc ) => {
						let slice = &arc[..];
						#(#convert);*;
						let junctions: Junctions = [#(#idents),*].into();
						junctions
					},
				}
			})
			.collect::<TokenStream>();

		quote! {
			impl core::convert::TryFrom<crate::v4::Junctions> for Junctions {
				type Error = ();
				fn try_from(mut old: crate::v4::Junctions) -> core::result::Result<Self, ()> {
					Ok(match old {
					 crate::v4::Junctions::Here => Junctions::Here,
					 #match_variants
					})
				}
			}
		}
	}
}
