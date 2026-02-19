#![forbid(unsafe_code)]

use heck::{
	ToPascalCase,
	ToSnakeCase,
};
use proc_macro::{
	Span,
	TokenStream,
};
use proc_macro_crate::FoundCrate;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Will be used for future cascade behavior
enum CascadeType {
	Cascade,
	Restrict,
	SetNull,
}

#[derive(Debug, Clone)]
struct MarkerFields {
	delete_marker: Option<String>,
	update_marker: Option<String>,
	insert_marker: Option<String>,
}

impl MarkerFields {
	fn new() -> Self {
		Self {
			delete_marker: None,
			update_marker: None,
			insert_marker: None,
		}
	}
}

fn sqlxo_root() -> proc_macro2::TokenStream {
	match proc_macro_crate::crate_name("sqlxo") {
		Ok(FoundCrate::Itself) => quote!(sqlxo),
		Ok(FoundCrate::Name(name)) => {
			let ident = syn::Ident::new(&name, Span::call_site().into());
			quote!(#ident)
		}
		Err(_) => quote!(sqlxo),
	}
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

fn extract_marker_fields(
	fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
) -> syn::Result<MarkerFields> {
	let mut markers = MarkerFields::new();

	for field in fields.iter() {
		let field_name =
			field.ident.as_ref().unwrap().to_string().to_snake_case();

		for attr in &field.attrs {
			if !attr.path.is_ident("sqlxo") {
				continue;
			}

			let meta = attr.parse_meta().map_err(|_| {
				syn::Error::new_spanned(attr, "invalid #[sqlxo] attribute")
			})?;

			let list = match meta {
				Meta::List(list) => list,
				_ => continue,
			};

			for nested in list.nested {
				match nested {
					NestedMeta::Meta(Meta::Path(path))
						if path.is_ident("delete_marker") =>
					{
						if markers.delete_marker.is_some() {
							return Err(syn::Error::new_spanned(
								attr,
								"duplicate #[sqlxo(delete_marker)]",
							));
						}
						markers.delete_marker = Some(field_name.clone());
					}
					NestedMeta::Meta(Meta::Path(path))
						if path.is_ident("update_marker") =>
					{
						if markers.update_marker.is_some() {
							return Err(syn::Error::new_spanned(
								attr,
								"duplicate #[sqlxo(update_marker)]",
							));
						}
						markers.update_marker = Some(field_name.clone());
					}
					NestedMeta::Meta(Meta::Path(path))
						if path.is_ident("insert_marker") =>
					{
						if markers.insert_marker.is_some() {
							return Err(syn::Error::new_spanned(
								attr,
								"duplicate #[sqlxo(insert_marker)]",
							));
						}
						markers.insert_marker = Some(field_name.clone());
					}
					_ => {}
				}
			}
		}
	}

	Ok(markers)
}

struct FkSpec {
	fk_field_snake:  String,
	fk_field_pascal: Ident,
	right_table:     String,
	right_pk:        String,
	#[allow(dead_code)] // Will be used for future cascade behavior
	cascade_type: Option<CascadeType>,
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
	let root = sqlxo_root();
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

		variants.push(quote! { #var(#root::JoinKind) });

		let right_table = fk.right_table.clone();
		let on_left = format!(r#""{}"."{}""#, left_table, fk.fk_field_snake);
		let on_right = format!(r#""{}"."{}""#, right_table, fk.right_pk);

		to_sql.push(quote! {
			Self::#var(kind) => match kind {
				#root::JoinKind::Inner =>
					format!(r#" INNER JOIN {} ON {} = {}"#, #right_table, #on_left, #on_right),
				#root::JoinKind::Left  =>
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
	let root = sqlxo_root();

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
	let column_mod_ident = format_ident!("{}Column", struct_ident);

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
	let mut column_structs = Vec::new();

	let mut pk_field: Option<String> = None;
	let mut fks: Vec<FkSpec> = Vec::new();

	for field in fields.iter() {
		let field_ident = field.ident.clone().unwrap();
		let field_name_pascal = field_ident.to_string().to_pascal_case();
		let field_name_snake = field_ident.to_string().to_snake_case();
		let ty = &field.ty;
		let column_struct_ident = format_ident!("{}", field_name_pascal);

		column_structs.push(quote! {
			#[derive(Debug, Clone, Copy, PartialEq, Eq)]
			pub struct #column_struct_ident;

			impl #root::select::Column for #column_struct_ident {
				type Model = #struct_ident;
				type Type = #ty;
				const NAME: &'static str = #field_name_snake;
			}
		});

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
				let mut cascade_type: Option<CascadeType> = None;

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
						NestedMeta::Meta(Meta::List(inner_list))
							if inner_list.path.is_ident("cascade_type") =>
						{
							for inner_nested in inner_list.nested {
								if let NestedMeta::Meta(Meta::Path(path)) =
									inner_nested
								{
									if path.is_ident("cascade") {
										cascade_type =
											Some(CascadeType::Cascade);
									} else if path.is_ident("restrict") {
										cascade_type =
											Some(CascadeType::Restrict);
									} else if path.is_ident("set_null") {
										cascade_type =
											Some(CascadeType::SetNull);
									} else {
										return Error::new_spanned(
											path,
											"unknown cascade type; expected \
											 cascade, restrict, or set_null",
										)
										.to_compile_error()
										.into();
									}
								}
							}
						}
						other => {
							return Error::new_spanned(
								other,
								r#"unknown key; expected `to = "table.pk"` or `cascade_type(...)`"#,
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
					cascade_type,
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

				write_arms.push(quote! { Self::#v_eq(v)      => { w.push(concat!(#col, " = "));       w.bind(v.clone()); } });
				write_arms.push(quote! { Self::#v_neq(v)     => { w.push(concat!(#col, " <> "));      w.bind(v.clone()); } });
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

		impl #root::QueryContext for #struct_ident {
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


		impl #root::SqlJoin for #join_ident {
			fn to_sql(&self) -> String {
				match self {
					#(#join_to_sql_arms),*
				}
			}

			fn kind(&self) -> #root::JoinKind {
				match self {
					#(#join_kind_arms),*
				}
			}
		}


		impl #root::Filterable for #query_ident {
			type Entity = #struct_ident;

			fn write<W: #root::SqlWrite>(&self, w: &mut W) {
				match self {
					#(#write_arms),*
				}
			}
		}


		impl #root::Sortable for #sort_ident {
			type Entity = #struct_ident;

			fn sort_clause(&self) -> String {
				match self {
					#(#sort_sql_arms),*
				}
			}
		}

		impl ::std::iter::IntoIterator for #sort_ident {
			type Item = #sort_ident;
			type IntoIter = ::std::iter::Once<#sort_ident>;

			fn into_iter(self) -> Self::IntoIter {
				::std::iter::once(self)
			}
		}

		#[allow(non_snake_case)]
		pub mod #column_mod_ident {
			use super::*;
			#(#column_structs)*
		}
	};

	out.into()
}

#[proc_macro]
pub fn context(input: TokenStream) -> TokenStream {
	let parser = syn::punctuated::Punctuated::<syn::Type, syn::Token![,]>::parse_terminated;
	let args = parse_macro_input!(input with parser);
	let root = sqlxo_root();

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

			impl #root::QueryContext for Ctx {
				const TABLE: &'static str = <#base_ty as #root::QueryContext>::TABLE;

				type Model = <#base_ty as #root::QueryContext>::Model;
				type Query = <#base_ty as #root::QueryContext>::Query;
				type Sort  = <#base_ty as #root::QueryContext>::Sort;
				type Join  = <#base_ty as #root::QueryContext>::Join;
			}

			pub type Where = <#base_ty as #root::QueryContext>::Query;
			pub type Sort  = <#base_ty as #root::QueryContext>::Sort;
			pub type Join  = <#base_ty as #root::QueryContext>::Join;

			pub use #root::JoinKind;
		}

	};

	out.into()
}

#[proc_macro_derive(WebQuery, attributes(sqlxo, primary_key, foreign_key))]
pub fn derive_webquery(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let root = sqlxo_root();

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

		let mut webquery_ignore = false;
		let mut bool_field: Option<String> = None;

		for attr in &f.attrs {
			if attr.path.is_ident("sqlxo") {
				if let Ok(Meta::List(list)) = attr.parse_meta() {
					for nested in list.nested {
						match nested {
							NestedMeta::Meta(Meta::Path(p))
								if p.is_ident("webquery_ignore") =>
							{
								webquery_ignore = true;
							}

							NestedMeta::Meta(Meta::NameValue(nv))
								if nv.path.is_ident("bool_from_nullable") =>
							{
								if let Lit::Str(ref s) = nv.lit {
									bool_field = Some(s.value());
								}
							}

							_ => {}
						}
					}
				}
			}
		}

		if webquery_ignore {
			continue;
		}

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

			Kind::Bool => {
				let doc_attr = if let Some(bf) = bool_field {
					format!(
						"This boolean maps to the presence of `{}` (IS NOT \
						 NULL / IS NULL).",
						bf
					)
				} else {
					"Boolean filter".to_string()
				};

				quote! {
					#[doc = #doc_attr]
					#[derive(Clone, serde::Serialize, serde::Deserialize, utoipa::ToSchema, Debug)]
					#[serde(untagged)]
					pub enum #op_ident {
						IsTrue  { is_true: bool },
						IsFalse { is_false: bool },
					}
				}
			}

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
                pub #fname_ident: #root::WebSortDirection,
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


		impl #root::WebQueryModel for #struct_ident {
			type Leaf      = #leaf_ident;
			type SortField = #sort_field_ident;
		}

	};

	out.into()
}

#[derive(Debug, Clone, PartialEq)]
enum PrimaryKeyMode {
	Manual,
	GeneratedUuid,
	GeneratedSequence(String),
}

fn extract_primary_key_mode(
	field: &syn::Field,
) -> syn::Result<Option<PrimaryKeyMode>> {
	for attr in &field.attrs {
		if !attr.path.is_ident("primary_key") {
			continue;
		}

		// Check if it's just #[primary_key] with no arguments
		let meta = match attr.parse_meta() {
			Ok(meta) => meta,
			Err(_) => {
				// If parsing fails, treat as simple #[primary_key]
				return Ok(Some(PrimaryKeyMode::Manual));
			}
		};

		match meta {
			// #[primary_key] with no args
			Meta::Path(_) => {
				return Ok(Some(PrimaryKeyMode::Manual));
			}
			// #[primary_key(manual)] or #[primary_key(generated(...))]
			Meta::List(list) => {
				for nested in list.nested {
					match nested {
						NestedMeta::Meta(Meta::Path(path))
							if path.is_ident("manual") =>
						{
							return Ok(Some(PrimaryKeyMode::Manual));
						}
						NestedMeta::Meta(Meta::List(inner_list))
							if inner_list.path.is_ident("generated") =>
						{
							if let Some(inner_nested) =
								inner_list.nested.into_iter().next()
							{
								match inner_nested {
									NestedMeta::Meta(Meta::Path(path))
										if path.is_ident("uuid") =>
									{
										return Ok(Some(
											PrimaryKeyMode::GeneratedUuid,
										));
									}
									NestedMeta::Meta(Meta::NameValue(nv))
										if nv.path.is_ident("sequence") =>
									{
										match nv.lit {
											Lit::Str(ref s) => {
												return Ok(Some(PrimaryKeyMode::GeneratedSequence(
													s.value(),
												)));
											}
											other => {
												return Err(
													syn::Error::new_spanned(
														other,
														r#"expected string literal: #[primary_key(generated(sequence = "seq_name"))]"#,
													),
												);
											}
										}
									}
									other => {
										return Err(syn::Error::new_spanned(
											other,
											r#"expected `uuid` or `sequence = "..."`"#,
										));
									}
								}
							}
						}
						other => {
							return Err(syn::Error::new_spanned(
								other,
								r#"expected `manual` or `generated(...)`"#,
							));
						}
					}
				}
			}
			other => {
				return Err(syn::Error::new_spanned(
					other,
					r#"expected #[primary_key], #[primary_key(manual)], or #[primary_key(generated(...))]"#,
				));
			}
		}
	}

	Ok(None)
}

#[proc_macro_derive(Create, attributes(sqlxo, primary_key, foreign_key))]
pub fn derive_create(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let root = sqlxo_root();

	let struct_ident = &input.ident;
	let create_ident = format_ident!("{}Creation", struct_ident);

	let data = match &input.data {
		Data::Struct(s) => s,
		_ => {
			return Error::new_spanned(
				&input.ident,
				"`Create` supports only structs",
			)
			.to_compile_error()
			.into();
		}
	};

	let fields = match &data.fields {
		Fields::Named(named) => &named.named,
		other => {
			return Error::new_spanned(other, "`Create` requires named fields")
				.to_compile_error()
				.into();
		}
	};

	let markers = match extract_marker_fields(fields) {
		Ok(m) => m,
		Err(e) => return e.to_compile_error().into(),
	};

	// Collect primary key fields and their modes
	let mut pk_fields = std::collections::HashMap::new();
	for field in fields.iter() {
		match extract_primary_key_mode(field) {
			Ok(Some(mode)) => {
				let field_name = field.ident.as_ref().unwrap().to_string();
				pk_fields.insert(field_name, mode);
			}
			Ok(None) => {}
			Err(e) => return e.to_compile_error().into(),
		}
	}

	// Generate create struct fields
	let mut create_fields = Vec::new();
	let mut field_names = Vec::new();
	let mut field_names_snake = Vec::new();

	for field in fields.iter() {
		let field_ident = field.ident.as_ref().unwrap();
		let field_name = field_ident.to_string();
		let field_name_snake = field_name.to_snake_case();
		let ty = &field.ty;

		// Skip generated primary keys
		if let Some(mode) = pk_fields.get(&field_name) {
			match mode {
				PrimaryKeyMode::GeneratedUuid |
				PrimaryKeyMode::GeneratedSequence(_) => {
					continue;
				}
				PrimaryKeyMode::Manual => {
					// Include manual primary keys
				}
			}
		}

		// Skip all marker fields
		if Some(&field_name_snake) == markers.delete_marker.as_ref() ||
			Some(&field_name_snake) == markers.update_marker.as_ref() ||
			Some(&field_name_snake) == markers.insert_marker.as_ref()
		{
			continue;
		}

		create_fields.push(quote! {
			pub #field_ident: #ty
		});
		field_names.push(field_ident);
		field_names_snake.push(field_name_snake);
	}

	let insert_marker_field = markers
		.insert_marker
		.map(|f| quote! { Some(#f) })
		.unwrap_or_else(|| quote! { None });

	let out = quote! {
		#[derive(Debug, Clone)]
		pub struct #create_ident {
			#(#create_fields),*
		}

		impl #root::Creatable for #struct_ident {
			type CreateModel = #create_ident;
			const INSERT_MARKER_FIELD: Option<&'static str> = #insert_marker_field;
		}

		impl #root::CreateModel for #create_ident {
			type Entity = #struct_ident;

			fn apply_inserts(
				&self,
				qb: &mut sqlx::QueryBuilder<'static, sqlx::Postgres>,
				insert_marker_field: Option<&'static str>,
			) {
				// Build column list
				qb.push("(");
				let mut first = true;

				#(
					if !first {
						qb.push(", ");
					}
					first = false;
					qb.push(#field_names_snake);
				)*

				// Add insert marker column if present
				if let Some(marker) = insert_marker_field {
					if !first {
						qb.push(", ");
					}
					qb.push(marker);
				}

				qb.push(") VALUES (");

				// Build values list with proper bindings
				let mut first = true;

				#(
					if !first {
						qb.push(", ");
					}
					first = false;
					qb.push_bind(self.#field_names.clone());
				)*

				// Add insert marker value if present
				if insert_marker_field.is_some() {
					if !first {
						qb.push(", ");
					}
					qb.push("NOW()");
				}

				qb.push(")");
			}
		}
	};

	out.into()
}

#[proc_macro_attribute]
pub fn bind(attr: TokenStream, item: TokenStream) -> TokenStream {
	let dto = parse_macro_input!(item as DeriveInput);
	let root = sqlxo_root();

	let entity_ty: syn::Type = {
		let s = attr.to_string();
		let s = s.trim();
		let cleaned = if let Some(stripped) = s.strip_prefix('=') {
			stripped
		} else {
			s
		};
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
				dto_ident,
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

		let mut target_snake = fname_snake.clone();
		let mut webquery_ignore = false;

		for attr in &field.attrs {
			if attr.path.is_ident("sqlxo") {
				let meta = match attr.parse_meta() {
					Ok(m) => m,
					Err(_) => {
						return Error::new_spanned(
							attr,
							"invalid #[sqlxo] attribute",
						)
						.to_compile_error()
						.into();
					}
				};
				let list = match meta {
					Meta::List(list) => list,
					_ => {
						return Error::new_spanned(
							attr,
							r#"expected #[sqlxo(field = "...")]"#,
						)
						.to_compile_error()
						.into();
					}
				};
				for nested in list.nested {
					match nested {
						NestedMeta::Meta(Meta::NameValue(nv))
							if nv.path.is_ident("field") =>
						{
							match nv.lit {
								Lit::Str(ref s) => {
									target_snake = s.value();
								}
								other => {
									return Error::new_spanned(
										other,
										r#"expected string literal: #[sqlxo(field = "item_name")]"#,
									)
									.to_compile_error()
									.into();
								}
							}
						}
						NestedMeta::Meta(Meta::Path(p))
							if p.is_ident("webquery_ignore") ||
								p.is_ident("webquer_ignore") =>
						{
							webquery_ignore = true;
						}
						// optional: #[sqlxo(webquery_ignore = true)]
						NestedMeta::Meta(Meta::NameValue(nv))
							if nv.path.is_ident("webquery_ignore") =>
						{
							match nv.lit {
								Lit::Bool(b) => webquery_ignore = b.value,
								other => {
									return Error::new_spanned(
										other,
										r#"expected bool literal: #[sqlxo(webquery_ignore = true)]"#,
									)
									.to_compile_error()
									.into();
								}
							}
						}
						NestedMeta::Meta(Meta::NameValue(nv)) => {
							return Error::new_spanned(
								nv,
								"unknown key in #[sqlxo]",
							)
							.to_compile_error()
							.into();
						}
						other => {
							return Error::new_spanned(
								other,
								"expected name-value pair",
							)
							.to_compile_error()
							.into();
						}
					}
				}
			}
		}

		if webquery_ignore {
			continue;
		}
		let target_pascal = target_snake.to_pascal_case();

		let leaf_wrap_ident =
			format_ident!("{}Leaf{}", dto_ident, fname_pascal);
		let sort_wrap_ident =
			format_ident!("{}Sort{}", dto_ident, fname_pascal);
		let op_ident = format_ident!("{}{}Op", dto_ident, fname_pascal);

		let leaf_variant_ident = format_ident!("{}", fname_pascal);
		let sort_variant_ident = format_ident!("{}", fname_pascal);

		let q_eq = format_ident!("{}Eq", target_pascal);
		let q_neq = format_ident!("{}Neq", target_pascal);
		let q_like = format_ident!("{}Like", target_pascal);
		let q_not_like = format_ident!("{}NotLike", target_pascal);
		let q_is_null = format_ident!("{}IsNull", target_pascal);
		let q_is_notnull = format_ident!("{}IsNotNull", target_pascal);
		let q_gt = format_ident!("{}Gt", target_pascal);
		let q_gte = format_ident!("{}Gte", target_pascal);
		let q_lt = format_ident!("{}Lt", target_pascal);
		let q_lte = format_ident!("{}Lte", target_pascal);
		let q_between = format_ident!("{}Between", target_pascal);
		let q_not_between = format_ident!("{}NotBetween", target_pascal);
		let q_is_true = format_ident!("{}IsTrue", target_pascal);
		let q_is_false = format_ident!("{}IsFalse", target_pascal);

		let s_by_asc = format_ident!("By{}Asc", target_pascal);
		let s_by_desc = format_ident!("By{}Desc", target_pascal);

		match classify_type(ty) {
			Kind::String => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::Eq{eq: v}            => <#entity_ty as #root::QueryContext>::Query::#q_eq(v.clone()),
                            #op_ident::Neq{neq: v}          => <#entity_ty as #root::QueryContext>::Query::#q_neq(v.clone()),
                            #op_ident::Like{like: v}        => <#entity_ty as #root::QueryContext>::Query::#q_like(v.clone()),
                            #op_ident::NotLike{not_like: v} => <#entity_ty as #root::QueryContext>::Query::#q_not_like(v.clone()),
                            #op_ident::IsNull{..}           => <#entity_ty as #root::QueryContext>::Query::#q_is_null,
                            #op_ident::IsNotNull{..}        => <#entity_ty as #root::QueryContext>::Query::#q_is_notnull,
                        }
                    }
                });
			}
			Kind::Bool => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::IsTrue{..}  => <#entity_ty as #root::QueryContext>::Query::#q_is_true,
                            #op_ident::IsFalse{..} => <#entity_ty as #root::QueryContext>::Query::#q_is_false,
                        }
                    }
                });
			}
			Kind::Number => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::Eq{eq: v}            => <#entity_ty as #root::QueryContext>::Query::#q_eq(*v),
                            #op_ident::Neq{neq: v}          => <#entity_ty as #root::QueryContext>::Query::#q_neq(*v),
                            #op_ident::Gt{gt: v}            => <#entity_ty as #root::QueryContext>::Query::#q_gt(*v),
                            #op_ident::Gte{gte: v}          => <#entity_ty as #root::QueryContext>::Query::#q_gte(*v),
                            #op_ident::Lt{lt: v}            => <#entity_ty as #root::QueryContext>::Query::#q_lt(*v),
                            #op_ident::Lte{lte: v}          => <#entity_ty as #root::QueryContext>::Query::#q_lte(*v),
                            #op_ident::Between{between: v}  => <#entity_ty as #root::QueryContext>::Query::#q_between(v[0], v[1]),
                            #op_ident::NotBetween{not_between: v}
                                                            => <#entity_ty as #root::QueryContext>::Query::#q_not_between(v[0], v[1]),
                        }
                    }
                });
			}
			Kind::UuidOrScalarEq => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::Eq{eq: v}            => <#entity_ty as #root::QueryContext>::Query::#q_eq(v.clone()),
                            #op_ident::Neq{neq: v}          => <#entity_ty as #root::QueryContext>::Query::#q_neq(v.clone()),
                            #op_ident::IsNull{..}           => <#entity_ty as #root::QueryContext>::Query::#q_is_null,
                            #op_ident::IsNotNull{..}        => <#entity_ty as #root::QueryContext>::Query::#q_is_notnull,
                        }
                    }
                });
			}
			Kind::DateTime => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::On{on: v}            => <#entity_ty as #root::QueryContext>::Query::#q_eq(v.clone()),
                            #op_ident::Eq{eq: v}            => <#entity_ty as #root::QueryContext>::Query::#q_eq(v.clone()),
                            #op_ident::Neq{neq: v}          => <#entity_ty as #root::QueryContext>::Query::#q_neq(v.clone()),
                            #op_ident::Gt{gt: v}            => <#entity_ty as #root::QueryContext>::Query::#q_gt(v.clone()),
                            #op_ident::Gte{gte: v}          => <#entity_ty as #root::QueryContext>::Query::#q_gte(v.clone()),
                            #op_ident::Lt{lt: v}            => <#entity_ty as #root::QueryContext>::Query::#q_lt(v.clone()),
                            #op_ident::Lte{lte: v}          => <#entity_ty as #root::QueryContext>::Query::#q_lte(v.clone()),
                            #op_ident::Between{between: v}  => <#entity_ty as #root::QueryContext>::Query::#q_between(v[0].clone(), v[1].clone()),
                            #op_ident::NotBetween{not_between: v}
                                                            => <#entity_ty as #root::QueryContext>::Query::#q_not_between(v[0].clone(), v[1].clone()),
                            #op_ident::IsNull{..}           => <#entity_ty as #root::QueryContext>::Query::#q_is_null,
                            #op_ident::IsNotNull{..}        => <#entity_ty as #root::QueryContext>::Query::#q_is_notnull,
                        }
                    }
                });
			}
			Kind::Date | Kind::Time => {
				leaf_arms.push(quote! {
                    #leaf_ident::#leaf_variant_ident(inner @ #leaf_wrap_ident { .. }) => {
                        match &inner.#fname_ident {
                            #op_ident::Eq{eq: v}            => <#entity_ty as #root::QueryContext>::Query::#q_eq(v.clone()),
                            #op_ident::Neq{neq: v}          => <#entity_ty as #root::QueryContext>::Query::#q_neq(v.clone()),
                            #op_ident::Gt{gt: v}            => <#entity_ty as #root::QueryContext>::Query::#q_gt(v.clone()),
                            #op_ident::Gte{gte: v}          => <#entity_ty as #root::QueryContext>::Query::#q_gte(v.clone()),
                            #op_ident::Lt{lt: v}            => <#entity_ty as #root::QueryContext>::Query::#q_lt(v.clone()),
                            #op_ident::Lte{lte: v}          => <#entity_ty as #root::QueryContext>::Query::#q_lte(v.clone()),
                            #op_ident::Between{between: v}  => <#entity_ty as #root::QueryContext>::Query::#q_between(v[0].clone(), v[1].clone()),
                            #op_ident::NotBetween{not_between: v}
                                                            => <#entity_ty as #root::QueryContext>::Query::#q_not_between(v[0].clone(), v[1].clone()),
                            #op_ident::IsNull{..}           => <#entity_ty as #root::QueryContext>::Query::#q_is_null,
                            #op_ident::IsNotNull{..}        => <#entity_ty as #root::QueryContext>::Query::#q_is_notnull,
                        }
                    }
                });
			}
		}

		sort_arms.push(quote! {
            #sort_field_ident::#sort_variant_ident(inner @ #sort_wrap_ident { .. }) => {
                match inner.#fname_ident {
                    #root::WebSortDirection::Asc  => <#entity_ty as #root::QueryContext>::Sort::#s_by_asc,
                    #root::WebSortDirection::Desc => <#entity_ty as #root::QueryContext>::Sort::#s_by_desc,
                }
            }
        });
	}

	let out = quote! {
			#dto

	impl #root::Bind<#entity_ty> for #dto_ident {
		fn map_leaf(
			leaf: &<#dto_ident as #root::WebQueryModel>::Leaf
		) -> <#entity_ty as #root::QueryContext>::Query {
			match leaf {
				#(#leaf_arms),* ,
			}
		}

		fn map_sort_field(
			sort: &<#dto_ident as #root::WebQueryModel>::SortField
		) -> <#entity_ty as #root::QueryContext>::Sort {
			match sort {
				#(#sort_arms),* ,
			}
		}
	}
		};

	out.into()
}

#[proc_macro_derive(Delete, attributes(sqlxo, primary_key, foreign_key))]
pub fn derive_delete(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let root = sqlxo_root();

	let struct_ident = &input.ident;

	let out = quote! {
		impl #root::Deletable for #struct_ident {
			const IS_SOFT_DELETE: bool = false;
			const DELETE_MARKER_FIELD: Option<&'static str> = None;
		}
	};

	out.into()
}

#[proc_macro_derive(SoftDelete, attributes(sqlxo, primary_key, foreign_key))]
pub fn derive_soft_delete(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let root = sqlxo_root();

	let struct_ident = &input.ident;

	let data = match &input.data {
		Data::Struct(s) => s,
		_ => {
			return Error::new_spanned(
				&input.ident,
				"`SoftDelete` supports only structs",
			)
			.to_compile_error()
			.into();
		}
	};

	let fields = match &data.fields {
		Fields::Named(named) => &named.named,
		other => {
			return Error::new_spanned(
				other,
				"`SoftDelete` requires named fields",
			)
			.to_compile_error()
			.into();
		}
	};

	let markers = match extract_marker_fields(fields) {
		Ok(m) => m,
		Err(e) => return e.to_compile_error().into(),
	};

	let delete_marker = match markers.delete_marker {
		Some(ref field) => quote! { Some(#field) },
		None => {
			return Error::new_spanned(
				&input.ident,
				"`SoftDelete` requires a field marked with \
				 #[sqlxo::delete_marker]",
			)
			.to_compile_error()
			.into();
		}
	};

	let out = quote! {
		impl #root::Deletable for #struct_ident {
			const IS_SOFT_DELETE: bool = true;
			const DELETE_MARKER_FIELD: Option<&'static str> = #delete_marker;
		}
	};

	out.into()
}

#[proc_macro_derive(Update, attributes(sqlxo, primary_key, foreign_key))]
pub fn derive_update(input: TokenStream) -> TokenStream {
	let input = parse_macro_input!(input as DeriveInput);
	let root = sqlxo_root();

	let struct_ident = &input.ident;
	let update_ident = format_ident!("{}Update", struct_ident);

	let data = match &input.data {
		Data::Struct(s) => s,
		_ => {
			return Error::new_spanned(
				&input.ident,
				"`Update` supports only structs",
			)
			.to_compile_error()
			.into();
		}
	};

	let fields = match &data.fields {
		Fields::Named(named) => &named.named,
		other => {
			return Error::new_spanned(other, "`Update` requires named fields")
				.to_compile_error()
				.into();
		}
	};

	let markers = match extract_marker_fields(fields) {
		Ok(m) => m,
		Err(e) => return e.to_compile_error().into(),
	};

	// Collect primary key fields
	let mut pk_fields = Vec::new();
	for field in fields.iter() {
		for attr in &field.attrs {
			if attr.path.is_ident("primary_key") {
				let field_name = field.ident.as_ref().unwrap().to_string();
				pk_fields.push(field_name);
			}
		}
	}

	// Generate update struct fields
	let mut update_fields = Vec::new();
	let mut field_names = Vec::new();

	for field in fields.iter() {
		let field_ident = field.ident.as_ref().unwrap();
		let field_name = field_ident.to_string();
		let ty = &field.ty;

		// Skip primary keys
		if pk_fields.contains(&field_name) {
			continue;
		}

		// Skip markers
		if Some(&field_name) == markers.delete_marker.as_ref() ||
			Some(&field_name) == markers.update_marker.as_ref() ||
			Some(&field_name) == markers.insert_marker.as_ref()
		{
			continue;
		}

		// Skip fields marked as update_ignore
		let mut update_ignore = false;
		for attr in &field.attrs {
			if !attr.path.is_ident("sqlxo") {
				continue;
			}

			let meta = match attr.parse_meta() {
				Ok(m) => m,
				Err(_) => {
					return Error::new_spanned(
						attr,
						"invalid #[sqlxo] attribute",
					)
					.to_compile_error()
					.into();
				}
			};

			let list = match meta {
				Meta::List(list) => list,
				_ => continue,
			};

			for nested in list.nested {
				if let NestedMeta::Meta(Meta::Path(path)) = nested {
					if path.is_ident("update_ignore") {
						update_ignore = true;
						break;
					}
				}
			}

			if update_ignore {
				break;
			}
		}

		if update_ignore {
			continue;
		}

		// Wrap field type in Option
		// If already Option<T>, wrap as Option<Option<T>>
		update_fields.push(quote! {
			pub #field_ident: Option<#ty>
		});
		field_names.push(field_ident);
	}

	let update_marker_field = markers
		.update_marker
		.map(|f| quote! { Some(#f) })
		.unwrap_or_else(|| quote! { None });

	let out = quote! {
		#[derive(Debug, Clone, Default)]
		pub struct #update_ident {
			#(#update_fields),*
		}

		impl #root::Updatable for #struct_ident {
			type UpdateModel = #update_ident;
			const UPDATE_MARKER_FIELD: Option<&'static str> = #update_marker_field;
		}

		impl #root::UpdateModel for #update_ident {
			type Entity = #struct_ident;

			fn apply_updates(&self, qb: &mut sqlx::QueryBuilder<'static, sqlx::Postgres>, has_previous: bool) -> Vec<&'static str> {
				let mut set_fields = Vec::new();
				let mut needs_comma = has_previous;

				#(
					if let Some(ref val) = self.#field_names {
						if needs_comma {
							qb.push(", ");
						}
						needs_comma = true;

						qb.push(stringify!(#field_names));
						qb.push(" = ");
						qb.push_bind(val.clone());
						set_fields.push(stringify!(#field_names));
					}
				)*

				set_fields
			}
		}
	};

	out.into()
}
