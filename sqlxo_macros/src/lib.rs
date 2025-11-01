#![forbid(unsafe_code)]

use heck::{
	ToPascalCase,
	ToSnakeCase,
};
use proc_macro::TokenStream;
use quote::{
	format_ident,
	quote,
};
use syn::{
	parse_macro_input,
	spanned::Spanned,
	Data,
	DeriveInput,
	Error,
	Fields,
	Ident,
	Lit,
	Meta,
	NestedMeta,
	Visibility,
};

enum Kind {
	String,
	Bool,
	Number,
	UuidOrScalarEq,
	DateTime,
	Date,
	Time,
}

fn classify_type(ty: &syn::Type) -> Kind {
	if let syn::Type::Path(type_path) = ty {
		if type_path.path.segments.last().unwrap().ident == "Option" {
			if let syn::PathArguments::AngleBracketed(args) =
				&type_path.path.segments.last().unwrap().arguments
			{
				if let Some(syn::GenericArgument::Type(t)) = args.args.first() {
					return classify_type(t);
				}
			}
		}

		let last = type_path.path.segments.last().unwrap().ident.to_string();

		match last.as_str() {
			"String" => return Kind::String,
			"bool" => return Kind::Bool,
			"Uuid" => return Kind::UuidOrScalarEq,
			_ => {}
		}

		let seg_names: Vec<String> = type_path
			.path
			.segments
			.iter()
			.map(|s| s.ident.to_string())
			.collect();

		if seg_names.contains(&"DateTime".to_string()) &&
			seg_names.iter().any(|s| s == "chrono")
		{
			return Kind::DateTime;
		}
		if seg_names.contains(&"NaiveDate".to_string()) &&
			seg_names.iter().any(|s| s == "chrono")
		{
			return Kind::Date;
		}
		if seg_names.contains(&"NaiveTime".to_string()) &&
			seg_names.iter().any(|s| s == "chrono")
		{
			return Kind::Time;
		}

		let ints = [
			"i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64",
			"u128", "isize", "usize",
		];
		let floats = ["f32", "f64"];

		if ints.contains(&last.as_str()) || floats.contains(&last.as_str()) {
			return Kind::Number;
		}
	}

	Kind::UuidOrScalarEq
}

fn extract_table_name(input: &DeriveInput) -> syn::Result<Option<String>> {
	let mut value: Option<String> = None;

	for attr in &input.attrs {
		if !attr.path.is_ident("sqlxo") {
			continue;
		}

		let meta = attr.parse_meta().map_err(|_| {
			syn::Error::new_spanned(attr, "invalid #[sqlxo] attribute")
		})?;

		let list = match meta {
			Meta::List(list) => list,
			_ => {
				return Err(syn::Error::new_spanned(
					attr,
					"expected #[sqlxo(table_name = \"...\")]",
				))
			}
		};

		for nested in list.nested {
			match nested {
				NestedMeta::Meta(Meta::NameValue(nv))
					if nv.path.is_ident("table_name") =>
				{
					match nv.lit {
						Lit::Str(ref s) => {
							if value.is_some() {
								return Err(syn::Error::new_spanned(
									nv,
									"duplicate key `table_name`",
								));
							}
							value = Some(s.value());
						}
						other => {
							return Err(syn::Error::new_spanned(
								other,
								"expected string literal: #[sqlxo(table_name \
								 = \"items\")]",
							));
						}
					}
				}
				NestedMeta::Meta(Meta::NameValue(nv)) => {
					return Err(syn::Error::new_spanned(
						nv,
						"unknown key in #[sqlxo]",
					));
				}
				other => {
					return Err(syn::Error::new_spanned(
						other,
						"expected name-value pair",
					));
				}
			}
		}
	}

	Ok(value)
}

struct FkSpec {
	fk_field_snake:  String,
	fk_field_pascal: Ident,
	right_table:     String,
	right_pk:        String,
}

fn build_join_codegen(
	left_struct: &Ident,
	left_table: &str,
	fks: &[FkSpec],
) -> (
	Vec<proc_macro2::TokenStream>,
	Vec<proc_macro2::TokenStream>,
	Vec<proc_macro2::TokenStream>,
) {
	let mut variants = Vec::new();
	let mut to_sql = Vec::new();
	let mut kind_arms = Vec::new();

	if fks.is_empty() {
		let never = format_ident!("__Never");
		variants.push(quote! { #never(::core::convert::Infallible) });
		to_sql.push(
			quote! { Self::#never(_) => unreachable!("no joins for this model") },
		);
		kind_arms.push(
			quote! { Self::#never(_) => unreachable!("no joins for this model") },
		);
		return (variants, to_sql, kind_arms);
	}

	for fk in fks {
		let right_pascal = fk.right_table.to_pascal_case();
		let var = format_ident!(
			"{}To{}By{}",
			left_struct,
			right_pascal,
			fk.fk_field_pascal
		);

		variants.push(quote! { #var(sqlxo_traits::JoinKind) });

		let right_table = fk.right_table.clone();
		let on_left = format!(r#""{}"."{}""#, left_table, fk.fk_field_snake);
		let on_right = format!(r#""{}"."{}""#, right_table, fk.right_pk);

		to_sql.push(quote! {
			Self::#var(kind) => match kind {
				sqlxo_traits::JoinKind::Inner =>
					format!(r#" INNER JOIN {} ON {} = {}"#, #right_table, #on_left, #on_right),
				sqlxo_traits::JoinKind::Left  =>
					format!(r#" LEFT JOIN {} ON {} = {}"#,  #right_table, #on_left, #on_right),
			}
		});

		kind_arms.push(quote! { Self::#var(k) => *k });
	}

	(variants, to_sql, kind_arms)
}

#[proc_macro_derive(Query, attributes(sqlxo, primary_key, foreign_key))]
pub fn derive_query(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);

	if !matches!(input.vis, Visibility::Public(_)) {
		return Error::new_spanned(
			&input.vis,
			"`Query` requires a `pub` struct",
		)
		.to_compile_error()
		.into();
	}

	let table_name = match extract_table_name(&input) {
		Ok(Some(v)) => v,
		Ok(None) => input.ident.to_string().to_snake_case(),
		Err(e) => return e.to_compile_error().into(),
	};

	let struct_ident = &input.ident;
	let query_ident = format_ident!("{}Query", struct_ident);
	let sort_ident = format_ident!("{}Sort", struct_ident);
	let join_ident = format_ident!("{}Join", struct_ident);

	let data = match &input.data {
		Data::Struct(s) => s,
		_ => {
			return Error::new_spanned(
				&input.ident,
				"`Query` supports only structs",
			)
			.to_compile_error()
			.into();
		}
	};

	let fields = match &data.fields {
		Fields::Named(named) => &named.named,
		other => {
			return Error::new_spanned(other, "`Query` requires named fields")
				.to_compile_error()
				.into();
		}
	};

	let mut query_variants = Vec::new();
	let mut sort_variants = Vec::new();
	let mut write_arms = Vec::new();
	let mut sort_sql_arms = Vec::new();

	let mut pk_field: Option<String> = None;
	let mut fks: Vec<FkSpec> = Vec::new();

	for field in fields.iter() {
		let field_ident = field.ident.clone().unwrap();
		let field_name_pascal = field_ident.to_string().to_pascal_case();
		let field_name_snake = field_ident.to_string().to_snake_case();
		let ty = &field.ty;

		for attr in &field.attrs {
			if attr.path.is_ident("primary_key") {
				if pk_field.is_some() {
					return Error::new(attr.span(), "duplicate #[primary_key]")
						.to_compile_error()
						.into();
				}
				pk_field = Some(field_name_snake.clone());
			}

			if attr.path.is_ident("foreign_key") {
				let meta = attr
					.parse_meta()
					.map_err(|_| {
						Error::new_spanned(attr, "invalid #[foreign_key]")
					})
					.unwrap();

				let list = match meta {
					Meta::List(list) => list,
					_ => {
						return Error::new_spanned(
							attr,
							r#"expected #[foreign_key(to = "table.pk")]"#,
						)
						.to_compile_error()
						.into()
					}
				};

				let mut to_value: Option<String> = None;

				for nested in list.nested {
					match nested {
						NestedMeta::Meta(Meta::NameValue(nv))
							if nv.path.is_ident("to") =>
						{
							match nv.lit {
								Lit::Str(ref s) => {
									if to_value.is_some() {
										return Error::new_spanned(
											nv,
											"duplicate key `to`",
										)
										.to_compile_error()
										.into();
									}
									to_value = Some(s.value());
								}
								other => {
									return Error::new_spanned(
										other,
										r#"expected "table.pk""#,
									)
									.to_compile_error()
									.into();
								}
							}
						}
						other => {
							return Error::new_spanned(
								other,
								r#"unknown key; only `to = "table.pk"`"#,
							)
							.to_compile_error()
							.into();
						}
					}
				}

				let to = to_value.expect(r#"missing `to = "table.pk"`"#);

				let mut parts = to.split('.');
				let right_table = parts
					.next()
					.ok_or_else(|| Error::new(attr.span(), "missing table"))
					.unwrap()
					.to_string();
				let right_pk = parts
					.next()
					.ok_or_else(|| Error::new(attr.span(), "missing pk"))
					.unwrap()
					.to_string();

				if parts.next().is_some() {
					return Error::new(
						attr.span(),
						r#"invalid `to` — expected "table.pk""#,
					)
					.to_compile_error()
					.into();
				}

				fks.push(FkSpec {
					fk_field_snake: field_name_snake.clone(),
					fk_field_pascal: format_ident!("{}", field_name_pascal),
					right_table,
					right_pk,
				});
			}
		}

		let asc = format_ident!("By{}Asc", field_name_pascal);
		let desc = format_ident!("By{}Desc", field_name_pascal);

		sort_variants.push(quote! { #asc });
		sort_variants.push(quote! { #desc });

		let col: &str = &field_name_snake;

		sort_sql_arms.push(quote! { Self::#asc  => format!("{} ASC",  #col) });
		sort_sql_arms.push(quote! { Self::#desc => format!("{} DESC", #col) });

		match classify_type(ty) {
			Kind::String => {
				let v_eq = format_ident!("{}Eq", field_name_pascal);
				let v_neq = format_ident!("{}Neq", field_name_pascal);
				let v_like = format_ident!("{}Like", field_name_pascal);
				let v_not_like = format_ident!("{}NotLike", field_name_pascal);
				let v_is_null = format_ident!("{}IsNull", field_name_pascal);
				let v_is_notnul =
					format_ident!("{}IsNotNull", field_name_pascal);

				query_variants.push(quote! { #v_eq(String) });
				query_variants.push(quote! { #v_neq(String)  });
				query_variants.push(quote! { #v_like(String) });
				query_variants.push(quote! { #v_not_like(String) });
				query_variants.push(quote! { #v_is_null      });
				query_variants.push(quote! { #v_is_notnul    });

				write_arms.push(quote! { Self::#v_eq(v)       => { w.push(concat!(#col, " = "));       w.bind(v.clone()); } });
				write_arms.push(quote! { Self::#v_neq(v)      => { w.push(concat!(#col, " <> "));      w.bind(v.clone()); } });
				write_arms.push(quote! { Self::#v_like(v)     => { w.push(concat!(#col, " LIKE "));    w.bind(v.clone()); } });
				write_arms.push(quote! { Self::#v_not_like(v) => { w.push(concat!(#col, " NOT LIKE "));w.bind(v.clone()); } });
				write_arms.push(quote! { Self::#v_is_null     => { w.push(concat!(#col, " IS NULL"));                         } });
				write_arms.push(quote! { Self::#v_is_notnul   => { w.push(concat!(#col, " IS NOT NULL"));                     } });
			}

			Kind::Bool => {
				let v_true = format_ident!("{}IsTrue", field_name_pascal);
				let v_false = format_ident!("{}IsFalse", field_name_pascal);

				query_variants.push(quote! { #v_true  });
				query_variants.push(quote! { #v_false });

				write_arms.push(
					quote! { Self::#v_true  => { w.push(concat!(#col, " = TRUE"));  } },
				);
				write_arms.push(
					quote! { Self::#v_false => { w.push(concat!(#col, " = FALSE")); } },
				);
			}

			Kind::Number => {
				let v_eq = format_ident!("{}Eq", field_name_pascal);
				let v_neq = format_ident!("{}Neq", field_name_pascal);
				let v_gt = format_ident!("{}Gt", field_name_pascal);
				let v_gte = format_ident!("{}Gte", field_name_pascal);
				let v_lt = format_ident!("{}Lt", field_name_pascal);
				let v_lte = format_ident!("{}Lte", field_name_pascal);
				let v_between = format_ident!("{}Between", field_name_pascal);
				let v_notbetween =
					format_ident!("{}NotBetween", field_name_pascal);

				query_variants.push(quote! { #v_eq(#ty)          });
				query_variants.push(quote! { #v_neq(#ty)         });
				query_variants.push(quote! { #v_gt(#ty)          });
				query_variants.push(quote! { #v_gte(#ty)         });
				query_variants.push(quote! { #v_lt(#ty)          });
				query_variants.push(quote! { #v_lte(#ty)         });
				query_variants.push(quote! { #v_between(#ty,#ty) });
				query_variants.push(quote! { #v_notbetween(#ty,#ty) });

				write_arms.push(
                    quote! { Self::#v_eq(v)  => { w.push(concat!(#col, " = "));  w.bind(*v); } },
                );
				write_arms.push(
                    quote! { Self::#v_neq(v) => { w.push(concat!(#col, " <> ")); w.bind(*v); } },
                );
				write_arms.push(
                    quote! { Self::#v_gt(v)  => { w.push(concat!(#col, " > "));  w.bind(*v); } },
                );
				write_arms.push(
                    quote! { Self::#v_gte(v) => { w.push(concat!(#col, " >= ")); w.bind(*v); } },
                );
				write_arms.push(
                    quote! { Self::#v_lt(v)  => { w.push(concat!(#col, " < "));  w.bind(*v); } },
                );
				write_arms.push(
                    quote! { Self::#v_lte(v) => { w.push(concat!(#col, " <= ")); w.bind(*v); } },
                );

				write_arms.push(quote! {
					Self::#v_between(a, b) => {
						w.push(concat!(#col, " BETWEEN "));
						w.bind(*a);
						w.push(" AND ");
						w.bind(*b);
					}
				});

				write_arms.push(quote! {
					Self::#v_notbetween(a, b) => {
						w.push(concat!(#col, " NOT BETWEEN "));
						w.bind(*a);
						w.push(" AND ");
						w.bind(*b);
					}
				});
			}

			Kind::UuidOrScalarEq => {
				let v_eq = format_ident!("{}Eq", field_name_pascal);
				let v_neq = format_ident!("{}Neq", field_name_pascal);
				let v_is_null = format_ident!("{}IsNull", field_name_pascal);
				let v_is_notnul =
					format_ident!("{}IsNotNull", field_name_pascal);

				query_variants.push(quote! { #v_eq(#ty) });
				query_variants.push(quote! { #v_neq(#ty) });
				query_variants.push(quote! { #v_is_null });
				query_variants.push(quote! { #v_is_notnul });

				write_arms.push(quote! { Self::#v_eq(v)      => { w.push(concat!(#col, " = "));       w.bind(*v); } });
				write_arms.push(quote! { Self::#v_neq(v)     => { w.push(concat!(#col, " <> "));      w.bind(*v); } });
				write_arms.push(quote! { Self::#v_is_null    => { w.push(concat!(#col, " IS NULL"));                 } });
				write_arms.push(quote! { Self::#v_is_notnul  => { w.push(concat!(#col, " IS NOT NULL"));             } });
			}

			Kind::DateTime => {
				let v_eq = format_ident!("{}Eq", field_name_pascal);
				let v_neq = format_ident!("{}Neq", field_name_pascal);
				let v_gt = format_ident!("{}Gt", field_name_pascal);
				let v_gte = format_ident!("{}Gte", field_name_pascal);
				let v_lt = format_ident!("{}Lt", field_name_pascal);
				let v_lte = format_ident!("{}Lte", field_name_pascal);
				let v_between = format_ident!("{}Between", field_name_pascal);
				let v_notbetween =
					format_ident!("{}NotBetween", field_name_pascal);
				let v_is_null = format_ident!("{}IsNull", field_name_pascal);
				let v_is_notnul =
					format_ident!("{}IsNotNull", field_name_pascal);

				query_variants.push(quote! { #v_eq(#ty)          });
				query_variants.push(quote! { #v_neq(#ty)         });
				query_variants.push(quote! { #v_gt(#ty)          });
				query_variants.push(quote! { #v_gte(#ty)         });
				query_variants.push(quote! { #v_lt(#ty)          });
				query_variants.push(quote! { #v_lte(#ty)         });
				query_variants.push(quote! { #v_between(#ty,#ty) });
				query_variants.push(quote! { #v_notbetween(#ty,#ty) });
				query_variants.push(quote! { #v_is_null });
				query_variants.push(quote! { #v_is_notnul });

				write_arms.push(quote! { Self::#v_eq(v)  => { w.push(concat!(#col, " = "));  w.bind(*v); } });
				write_arms.push(quote! { Self::#v_neq(v) => { w.push(concat!(#col, " <> ")); w.bind(*v); } });
				write_arms.push(quote! { Self::#v_gt(v)  => { w.push(concat!(#col, " > "));  w.bind(*v); } });
				write_arms.push(quote! { Self::#v_gte(v) => { w.push(concat!(#col, " >= ")); w.bind(*v); } });
				write_arms.push(quote! { Self::#v_lt(v)  => { w.push(concat!(#col, " < "));  w.bind(*v); } });
				write_arms.push(quote! { Self::#v_lte(v) => { w.push(concat!(#col, " <= ")); w.bind(*v); } });

				write_arms.push(quote! {
					Self::#v_between(a, b) => {
						w.push(concat!(#col, " BETWEEN "));
						w.bind(*a);
						w.push(" AND ");
						w.bind(*b);
					}
				});

				write_arms.push(quote! {
					Self::#v_notbetween(a, b) => {
						w.push(concat!(#col, " NOT BETWEEN "));
						w.bind(*a);
						w.push(" AND ");
						w.bind(*b);
					}
				});
				write_arms.push(quote! { Self::#v_is_null    => { w.push(concat!(#col, " IS NULL"));     } });
				write_arms.push(quote! { Self::#v_is_notnul  => { w.push(concat!(#col, " IS NOT NULL")); } });
			}

			Kind::Date | Kind::Time => {
				let v_eq = format_ident!("{}Eq", field_name_pascal);
				let v_neq = format_ident!("{}Neq", field_name_pascal);
				let v_gt = format_ident!("{}Gt", field_name_pascal);
				let v_gte = format_ident!("{}Gte", field_name_pascal);
				let v_lt = format_ident!("{}Lt", field_name_pascal);
				let v_lte = format_ident!("{}Lte", field_name_pascal);
				let v_between = format_ident!("{}Between", field_name_pascal);
				let v_notbetween =
					format_ident!("{}NotBetween", field_name_pascal);
				let v_is_null = format_ident!("{}IsNull", field_name_pascal);
				let v_is_notnul =
					format_ident!("{}IsNotNull", field_name_pascal);

				query_variants.push(quote! { #v_eq(#ty)          });
				query_variants.push(quote! { #v_neq(#ty)         });
				query_variants.push(quote! { #v_gt(#ty)          });
				query_variants.push(quote! { #v_gte(#ty)         });
				query_variants.push(quote! { #v_lt(#ty)          });
				query_variants.push(quote! { #v_lte(#ty)         });
				query_variants.push(quote! { #v_between(#ty,#ty) });
				query_variants.push(quote! { #v_notbetween(#ty,#ty) });
				query_variants.push(quote! { #v_is_null });
				query_variants.push(quote! { #v_is_notnul });

				write_arms.push(quote! { Self::#v_eq(v)  => { w.push(concat!(#col, " = "));  w.bind(*v); } });
				write_arms.push(quote! { Self::#v_neq(v) => { w.push(concat!(#col, " <> ")); w.bind(*v); } });
				write_arms.push(quote! { Self::#v_gt(v)  => { w.push(concat!(#col, " > "));  w.bind(*v); } });
				write_arms.push(quote! { Self::#v_gte(v) => { w.push(concat!(#col, " >= ")); w.bind(*v); } });
				write_arms.push(quote! { Self::#v_lt(v)  => { w.push(concat!(#col, " < "));  w.bind(*v); } });
				write_arms.push(quote! { Self::#v_lte(v) => { w.push(concat!(#col, " <= ")); w.bind(*v); } });

				write_arms.push(quote! {
					Self::#v_between(a, b) => {
						w.push(concat!(#col, " BETWEEN "));
						w.bind(*a);
						w.push(" AND ");
						w.bind(*b);
					}
				});

				write_arms.push(quote! {
					Self::#v_notbetween(a, b) => {
						w.push(concat!(#col, " NOT BETWEEN "));
						w.bind(*a);
						w.push(" AND ");
						w.bind(*b);
					}
				});
				write_arms.push(quote! { Self::#v_is_null    => { w.push(concat!(#col, " IS NULL"));     } });
				write_arms.push(quote! { Self::#v_is_notnul  => { w.push(concat!(#col, " IS NOT NULL")); } });
			}
		}
	}

	let (join_variants, join_to_sql_arms, join_kind_arms) =
		build_join_codegen(struct_ident, &table_name, &fks);

	let out = quote! {

		impl sqlxo_traits::QueryContext for #struct_ident {
			const TABLE: &'static str = #table_name;

			type Model = #struct_ident;
			type Query = #query_ident;
			type Sort  = #sort_ident;
			type Join  = #join_ident;
		}


		#[derive(Debug, Clone, PartialEq)]
		pub enum #query_ident {
			#(#query_variants),*
		}


		#[derive(Debug, Clone, Copy, PartialEq, Eq)]
		pub enum #sort_ident {
			#(#sort_variants),*
		}


		#[derive(Debug, Clone, Copy, PartialEq, Eq)]
		pub enum #join_ident {
			#(#join_variants),*
		}


		impl sqlxo_traits::SqlJoin for #join_ident {
			fn to_sql(&self) -> String {
				match self {
					#(#join_to_sql_arms),*
				}
			}

			fn kind(&self) -> sqlxo_traits::JoinKind {
				match self {
					#(#join_kind_arms),*
				}
			}
		}


		impl sqlxo_traits::Filterable for #query_ident {
			type Entity = #struct_ident;

			fn write<W: sqlxo_traits::SqlWrite>(&self, w: &mut W) {
				match self {
					#(#write_arms),*
				}
			}
		}


		impl sqlxo_traits::Sortable for #sort_ident {
			type Entity = #struct_ident;

			fn sort_clause(&self) -> String {
				match self {
					#(#sort_sql_arms),*
				}
			}
		}

	};

	out.into()
}

#[proc_macro]
pub fn context(input: TokenStream) -> TokenStream {
	let parser = syn::punctuated::Punctuated::<syn::Type, syn::Token![,]>::parse_terminated;
	let args = parse_macro_input!(input with parser);

	if args.is_empty() {
		return Error::new(
			proc_macro2::Span::call_site(),
			"context!(T1, [T2, ...]) requires at least one type",
		)
		.to_compile_error()
		.into();
	}

	let mut parts = Vec::new();

	for ty in args.iter() {
		let ident = match ty {
			syn::Type::Path(tp) => tp
				.path
				.segments
				.last()
				.map(|s| s.ident.to_string())
				.unwrap_or_else(|| "t".into()),
			_ => "t".into(),
		};
		parts.push(ident.to_snake_case());
	}

	let mod_name = format_ident!("ctx_{}", parts.join("_"));
	let base_ty = args.first().unwrap();

	let out = quote! {

		pub mod #mod_name {

			pub struct Ctx;

			impl sqlxo_traits::QueryContext for Ctx {
				const TABLE: &'static str = <#base_ty as sqlxo_traits::QueryContext>::TABLE;

				type Model = <#base_ty as sqlxo_traits::QueryContext>::Model;
				type Query = <#base_ty as sqlxo_traits::QueryContext>::Query;
				type Sort  = <#base_ty as sqlxo_traits::QueryContext>::Sort;
				type Join  = <#base_ty as sqlxo_traits::QueryContext>::Join;
			}

			pub type Where = <#base_ty as sqlxo_traits::QueryContext>::Query;
			pub type Sort  = <#base_ty as sqlxo_traits::QueryContext>::Sort;
			pub type Join  = <#base_ty as sqlxo_traits::QueryContext>::Join;

			pub use sqlxo_traits::JoinKind;
		}

	};

	out.into()
}

#[proc_macro_derive(WebQuery, attributes(sqlxo, primary_key, foreign_key))]
pub fn derive_webquery(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);

	if !matches!(input.vis, Visibility::Public(_)) {
		return Error::new_spanned(
			&input.ident,
			"`WebQuery` requires a public struct",
		)
		.to_compile_error()
		.into();
	}

	let struct_ident = &input.ident;

	let data = match &input.data {
		Data::Struct(s) => s,
		_ => {
			return Error::new_spanned(
				&input.ident,
				"`WebQuery` supports only structs",
			)
			.to_compile_error()
			.into();
		}
	};

	let fields = match &data.fields {
		Fields::Named(named) => &named.named,
		not_supported => {
			return Error::new_spanned(
				not_supported,
				"`WebQuery` requires named fields",
			)
			.to_compile_error()
			.into();
		}
	};

	let leaf_ident = format_ident!("{}Leaf", struct_ident);
	let sort_field_ident = format_ident!("{}SortField", struct_ident);

	let mut op_defs = Vec::new();
	let mut leaf_structs = Vec::new();
	let mut leaf_variants = Vec::new();
	let mut sort_structs = Vec::new();
	let mut sort_variants = Vec::new();

	for f in fields {
		let fname_ident = f.ident.as_ref().unwrap();
		let fname_snake = fname_ident.to_string();
		let fname_pascal = fname_snake.to_pascal_case();
		let ty = &f.ty;

		let op_ident = format_ident!("{}{}Op", struct_ident, fname_pascal);
		let leaf_wrap_ident =
			format_ident!("{}Leaf{}", struct_ident, fname_pascal);
		let sort_wrap_ident =
			format_ident!("{}Sort{}", struct_ident, fname_pascal);

		let leaf_variant_ident = format_ident!("{}", fname_pascal);
		let sort_variant_ident = format_ident!("{}", fname_pascal);

		let op_def = match classify_type(ty) {
			Kind::String => quote! {
				#[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, Debug)]
				#[serde(untagged)]
				pub enum #op_ident {
					Eq        { eq: String },
					Neq       { neq: String },
					Like      { like: String },
					NotLike   { not_like: String },
					IsNull    { is_null: bool },
					IsNotNull { is_not_null: bool },
				}
			},

			Kind::Bool => quote! {
				#[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, Debug)]
				#[serde(untagged)]
				pub enum #op_ident {
					IsTrue  { is_true: bool },
					IsFalse { is_false: bool },
				}
			},

			Kind::Number => quote! {
				#[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, Debug)]
				#[serde(untagged)]
				pub enum #op_ident {
					Eq         { eq: #ty },
					Neq        { neq: #ty },
					Gt         { gt: #ty },
					Gte        { gte: #ty },
					Lt         { lt: #ty },
					Lte        { lte: #ty },
					Between    { between: [#ty; 2] },
					NotBetween { not_between: [#ty; 2] },
				}
			},

			Kind::UuidOrScalarEq => quote! {
				#[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, Debug)]
				#[serde(untagged)]
				pub enum #op_ident {
					Eq        { eq: #ty },
					Neq       { neq: #ty },
					IsNull    { is_null: bool },
					IsNotNull { is_not_null: bool },
				}
			},

			Kind::DateTime => quote! {
				#[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, Debug)]
				#[serde(untagged)]
				pub enum #op_ident {
					// Backward-compatible alias; maps to Eq
					On         { on: #ty },
					Eq         { eq: #ty },
					Neq        { neq: #ty },
					Gt         { gt: #ty },
					Gte        { gte: #ty },
					Lt         { lt: #ty },
					Lte        { lte: #ty },
					Between    { between: [#ty; 2] },
					NotBetween { not_between: [#ty; 2] },
					IsNull     { is_null: bool },
					IsNotNull  { is_not_null: bool },
				}
			},

			Kind::Date | Kind::Time => quote! {
				#[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, Debug)]
				#[serde(untagged)]
				pub enum #op_ident {
					Eq         { eq: #ty },
					Neq        { neq: #ty },
					Gt         { gt: #ty },
					Gte        { gte: #ty },
					Lt         { lt: #ty },
					Lte        { lte: #ty },
					Between    { between: [#ty; 2] },
					NotBetween { not_between: [#ty; 2] },
					IsNull     { is_null: bool },
					IsNotNull  { is_not_null: bool },
				}
			},
		};

		op_defs.push(op_def);

		leaf_structs.push(quote! {
            #[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, Debug)]
            pub struct #leaf_wrap_ident {
                #[serde(rename = #fname_snake)]
                pub #fname_ident: #op_ident,
            }
        });

		leaf_variants.push(quote! {
			#leaf_variant_ident(#leaf_wrap_ident)
		});

		sort_structs.push(quote! {
            #[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, Debug)]
            pub struct #sort_wrap_ident {
                pub #fname_ident: ::sqlxo_traits::DtoSortDir,
            }
        });

		sort_variants.push(quote! {
			#sort_variant_ident(#sort_wrap_ident)
		});
	}

	let out = quote! {

		#(#op_defs)*


		#(#leaf_structs)*


		#[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, Debug)]
		#[serde(untagged)]
		pub enum #leaf_ident {
			#(#leaf_variants),*
		}


		#(#sort_structs)*


		#[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, Debug)]
		#[serde(untagged)]
		pub enum #sort_field_ident {
			#(#sort_variants),*
		}


		impl ::sqlxo_traits::WebQueryModel for #struct_ident {
			type Leaf      = #leaf_ident;
			type SortField = #sort_field_ident;
		}

	};

	out.into()
}

#[proc_macro_attribute]
pub fn bind(attr: TokenStream, item: TokenStream) -> TokenStream {
	let dto = parse_macro_input!(item as DeriveInput);

	let entity_ty: syn::Type = {
		let s = attr.to_string();
		let s = s.trim();
		let cleaned = if s.starts_with('=') { &s[1..] } else { s };
		syn::parse_str(cleaned.trim())
			.expect("bind attribute requires a target type, e.g. #[bind(Item)]")
	};

	let dto_ident = &dto.ident;
	let leaf_ident = format_ident!("{}Leaf", dto_ident);
	let sort_field_ident = format_ident!("{}SortField", dto_ident);

	let data = match &dto.data {
		Data::Struct(s) => s,
		_ => {
			return Error::new_spanned(
				&dto_ident,
				"`#[bind]` only supports structs",
			)
			.to_compile_error()
			.into();
		}
	};

	let mut leaf_arms = Vec::new();
	let mut sort_arms = Vec::new();

	for field in data.fields.iter() {
		let fname_ident = field.ident.as_ref().expect("named field");
		let fname_snake = fname_ident.to_string();
		let fname_pascal = fname_snake.to_pascal_case();
		let ty = &field.ty;

		let leaf_wrap_ident =
			format_ident!("{}Leaf{}", dto_ident, fname_pascal);
		let sort_wrap_ident =
			format_ident!("{}Sort{}", dto_ident, fname_pascal);
		let op_ident = format_ident!("{}{}Op", dto_ident, fname_pascal);

		let leaf_variant_ident = format_ident!("{}", fname_pascal);
		let sort_variant_ident = format_ident!("{}", fname_pascal);

		let q_eq = format_ident!("{}Eq", fname_pascal);
		let q_neq = format_ident!("{}Neq", fname_pascal);
		let q_like = format_ident!("{}Like", fname_pascal);
		let q_not_like = format_ident!("{}NotLike", fname_pascal);
		let q_is_null = format_ident!("{}IsNull", fname_pascal);
		let q_is_notnull = format_ident!("{}IsNotNull", fname_pascal);
		let q_gt = format_ident!("{}Gt", fname_pascal);
		let q_gte = format_ident!("{}Gte", fname_pascal);
		let q_lt = format_ident!("{}Lt", fname_pascal);
		let q_lte = format_ident!("{}Lte", fname_pascal);
		let q_between = format_ident!("{}Between", fname_pascal);
		let q_not_between = format_ident!("{}NotBetween", fname_pascal);
		let q_is_true = format_ident!("{}IsTrue", fname_pascal);
		let q_is_false = format_ident!("{}IsFalse", fname_pascal);

		let s_by_asc = format_ident!("By{}Asc", fname_pascal);
		let s_by_desc = format_ident!("By{}Desc", fname_pascal);

		match classify_type(ty) {
			Kind::String => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::Eq{eq: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_eq(v.clone()),
                            #op_ident::Neq{neq: v}          => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_neq(v.clone()),
                            #op_ident::Like{like: v}        => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_like(v.clone()),
                            #op_ident::NotLike{not_like: v} => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_not_like(v.clone()),
                            #op_ident::IsNull{..}           => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_is_null,
                            #op_ident::IsNotNull{..}        => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_is_notnull,
                        }
                    }
                });
			}
			Kind::Bool => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::IsTrue{..}  => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_is_true,
                            #op_ident::IsFalse{..} => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_is_false,
                        }
                    }
                });
			}
			Kind::Number => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::Eq{eq: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_eq(*v),
                            #op_ident::Neq{neq: v}          => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_neq(*v),
                            #op_ident::Gt{gt: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_gt(*v),
                            #op_ident::Gte{gte: v}          => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_gte(*v),
                            #op_ident::Lt{lt: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_lt(*v),
                            #op_ident::Lte{lte: v}          => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_lte(*v),
                            #op_ident::Between{between: v}  => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_between(v[0], v[1]),
                            #op_ident::NotBetween{not_between: v}
                                                            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_not_between(v[0], v[1]),
                        }
                    }
                });
			}
			Kind::UuidOrScalarEq => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::Eq{eq: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_eq(*v),
                            #op_ident::Neq{neq: v}          => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_neq(*v),
                            #op_ident::IsNull{..}           => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_is_null,
                            #op_ident::IsNotNull{..}        => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_is_notnull,
                        }
                    }
                });
			}
			Kind::DateTime => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::On{on: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_eq(*v),
                            #op_ident::Eq{eq: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_eq(*v),
                            #op_ident::Neq{neq: v}          => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_neq(*v),
                            #op_ident::Gt{gt: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_gt(*v),
                            #op_ident::Gte{gte: v}          => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_gte(*v),
                            #op_ident::Lt{lt: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_lt(*v),
                            #op_ident::Lte{lte: v}          => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_lte(*v),
                            #op_ident::Between{between: v}  => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_between(v[0], v[1]),
                            #op_ident::NotBetween{not_between: v}
                                                            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_not_between(v[0], v[1]),
                            #op_ident::IsNull{..}           => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_is_null,
                            #op_ident::IsNotNull{..}        => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_is_notnull,
                        }
                    }
                });
			}
			Kind::Date | Kind::Time => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::Eq{eq: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_eq(*v),
                            #op_ident::Neq{neq: v}          => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_neq(*v),
                            #op_ident::Gt{gt: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_gt(*v),
                            #op_ident::Gte{gte: v}          => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_gte(*v),
                            #op_ident::Lt{lt: v}            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_lt(*v),
                            #op_ident::Lte{lte: v}          => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_lte(*v),
                            #op_ident::Between{between: v}  => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_between(v[0], v[1]),
                            #op_ident::NotBetween{not_between: v}
                                                            => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_not_between(v[0], v[1]),
                            #op_ident::IsNull{..}           => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_is_null,
                            #op_ident::IsNotNull{..}        => <#entity_ty as sqlxo_traits::QueryContext>::Query::#q_is_notnull,
                        }
                    }
                });
			}
		}

		sort_arms.push(quote! {
            sqlxo_traits::GenericDtoSort(#sort_field_ident::#sort_variant_ident(inner @ #sort_wrap_ident { .. })) => {
                match inner.#fname_ident {
                    sqlxo_traits::DtoSortDir::Asc  => <#entity_ty as sqlxo_traits::QueryContext>::Sort::#s_by_asc,
                    sqlxo_traits::DtoSortDir::Desc => <#entity_ty as sqlxo_traits::QueryContext>::Sort::#s_by_desc,
                }
            }
        });
	}

	let out = quote! {
		#dto

		impl sqlxo_traits::Bind<#entity_ty> for #dto_ident {
			fn map_leaf(
				leaf: &<#dto_ident as sqlxo_traits::WebQueryModel>::Leaf
			) -> <#entity_ty as sqlxo_traits::QueryContext>::Query {
				match leaf {
					#(#leaf_arms),* ,
				}
			}

			fn map_sort_token(
				sort: &sqlxo_traits::DtoSort<Self>
			) -> <#entity_ty as sqlxo_traits::QueryContext>::Sort {
				match sort {
					#(#sort_arms),* ,
				}
			}
		}
	};

	out.into()
}
