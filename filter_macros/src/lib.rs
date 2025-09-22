#![feature(let_chains)]

use heck::{ToPascalCase, ToSnakeCase};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Data, DeriveInput, Error, Fields, Lit, Meta, NestedMeta, Visibility};

/// #[derive(GenerateQuerySort)]
/// optional: #[filter(table_name = "")]
#[proc_macro_derive(Query, attributes(filter))]
pub fn derive_queryt(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    if !matches!(input.vis, Visibility::Public(_)) {
        return Error::new_spanned(&input.vis, "`Query` derives requires a `pub` struct")
            .to_compile_error()
            .into();
    }

    let table_name = match extract_table_name(&input) {
        Ok(Some(v)) => v,
        Ok(None) => input.ident.to_string().to_snake_case(),
        Err(e) => return e.to_compile_error().into(),
    };

    let query_ident = format_ident!("{}Query", input.ident);
    let sort_ident = format_ident!("{}Sort", input.ident);

    let data = match &input.data {
        Data::Struct(s) => s,
        _not_supported => {
            return Error::new_spanned(&input.ident, "`Query` derive only supports structs")
                .to_compile_error()
                .into();
        }
    };

    let fields = match &data.fields {
        Fields::Named(named) => &named.named,
        not_supported => {
            return Error::new_spanned(
                not_supported,
                "`Query` derive requires a struct with named fields",
            )
            .to_compile_error()
            .into()
        }
    };

    let mut query_variants: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut sort_variants: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut sql_arms: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut bind_arms: Vec<proc_macro2::TokenStream> = Vec::new();

    for field in fields.iter() {
        let field_name = field.ident.clone().unwrap();
        let field_name_pascal = field_name.to_string().to_pascal_case();
        let field_name_snake = field_name.to_string().to_snake_case();
        let r#type = &field.ty;

        let asc = format_ident!("By{}Asc", field_name_pascal);
        let desc = format_ident!("By{}Desc", field_name_pascal);
        sort_variants.push(quote! { #asc });
        sort_variants.push(quote! { #desc });

        match classify_type(r#type) {
            Kind::String => {
                query_variants.extend(parse_string_operators(&field_name_pascal));
                add_string_impls(
                    &mut sql_arms,
                    &mut bind_arms,
                    &field_name_snake,
                    &field_name_pascal,
                );
            }
            Kind::Bool => {
                query_variants.extend(parse_boolean_operators(&field_name_pascal));
                add_bool_impls(
                    &mut sql_arms,
                    &mut bind_arms,
                    &field_name_snake,
                    &field_name_pascal,
                );
            }
            Kind::Number => {
                query_variants.extend(parse_numeric_operators(&field_name_pascal, r#type));
                add_numeric_impls(
                    &mut sql_arms,
                    &mut bind_arms,
                    &field_name_snake,
                    &field_name_pascal,
                );
            }
            Kind::UuidOrScalarEq => {
                query_variants.extend(parse_uuid_or_scalar_operators(&field_name_pascal, r#type));
                add_uuid_impls(
                    &mut sql_arms,
                    &mut bind_arms,
                    &field_name_snake,
                    &field_name_pascal,
                );
            }
            Kind::DateTime => {
                query_variants.extend(parse_datetime_operators(&field_name_pascal, r#type));
                add_datetime_impls(
                    &mut sql_arms,
                    &mut bind_arms,
                    &field_name_snake,
                    &field_name_pascal,
                );
            }
        }
    }
    let struct_ident = &input.ident;

    let expanded = quote! {
        impl filter_traits::QueryContext for #struct_ident {
            const TABLE: &'static str = #table_name;

            type Query = #query_ident;
            type Sort  = #sort_ident;
        }

        #[derive(Debug, Clone, PartialEq)]
        pub enum #query_ident {
            #(#query_variants),*
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum #sort_ident {
            #(#sort_variants),*
        }


        impl filter_traits::Filterable for #query_ident {
            type Entity = #struct_ident;

            fn filter_clause(&self, idx: &mut usize) -> String {
                match self {
                    #(#sql_arms),*
                }
            }

            fn bind<'q>(
                self,
                q: sqlx::query::QueryAs<
                    'q,
                    sqlx::Postgres,
                    Self::Entity,
                    sqlx::postgres::PgArguments
                >
            ) -> sqlx::query::QueryAs<
                'q,
                sqlx::Postgres,
                Self::Entity,
                sqlx::postgres::PgArguments
            > {
                match self {
                    #(#bind_arms),*
                }
            }
        }

        impl filter_traits::Sortable for #sort_ident { }
    };

    expanded.into()
}

/// Extracts `table_name` from `#[filter(table_name = "...")]`.
/// Returns:
/// - Ok(Some(name)) if provided,
/// - Ok(None) if the attribute is absent,
/// - Err(...) with a span-accurate, message.
fn extract_table_name(input: &DeriveInput) -> syn::Result<Option<String>> {
    let mut value: Option<String> = None;
    // let mut saw_filter_attr = false;

    for attr in &input.attrs {
        if !attr.path.is_ident("filter") {
            continue;
        }
        // saw_filter_attr = true;

        let meta = attr
            .parse_meta()
            .map_err(|_| syn::Error::new_spanned(attr, "invalid #[filter] attribute"))?;

        let list = match meta {
            Meta::List(list) => list,
            _ => {
                return Err(syn::Error::new_spanned(
                    attr,
                    "expected #[filter(table_name = \"...\")]", // wrong form
                ));
            }
        };

        for nested in list.nested {
            match nested {
                NestedMeta::Meta(Meta::NameValue(nv)) if nv.path.is_ident("table_name") => {
                    match nv.lit {
                        Lit::Str(ref s) => {
                            if value.is_some() {
                                return Err(syn::Error::new_spanned(
                                    nv,
                                    "duplicate key `table_name` in #[filter]",
                                ));
                            }
                            value = Some(s.value());
                        }
                        other => {
                            return Err(syn::Error::new_spanned(
                                other,
                                "expected string literal: #[filter(table_name = \"items\")]",
                            ));
                        }
                    }
                }
                NestedMeta::Meta(Meta::NameValue(nv)) => {
                    return Err(syn::Error::new_spanned(
                        nv,
                        "unknown key in #[filter]; only `table_name` is supported",
                    ));
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "expected name-value pair: table_name = \"...\"",
                    ));
                }
            }
        }
    }

    // if saw_filter_attr && value.is_none() {
    //     return Err(syn::Error::new_spanned(
    //         &input.ident,
    //         "missing `table_name` in #[filter]; expected #[filter(table_name = \"...\")]",
    //     ));
    // }

    Ok(value)
}

enum Kind {
    String,
    Bool,
    Number,
    UuidOrScalarEq,
    DateTime,
}

fn classify_type(r#type: &syn::Type) -> Kind {
    if let syn::Type::Path(type_path) = r#type {
        // properly parse Option<T>
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
        };

        // chrono::DateTime<Tz>
        let seg_names: Vec<String> = type_path
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        if seg_names.contains(&"DateTime".to_string()) && seg_names.iter().any(|s| s == "chrono") {
            return Kind::DateTime;
        }

        let ints = [
            "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128",
        ];
        let floats = ["f32", "f64"];
        if ints.contains(&last.as_str()) || floats.contains(&last.as_str()) {
            return Kind::Number;
        }
    }

    Kind::UuidOrScalarEq
}

fn parse_string_operators(field_name: &str) -> Vec<proc_macro2::TokenStream> {
    let v_eq = format_ident!("{}Eq", field_name);
    let v_neq = format_ident!("{}Neq", field_name);
    let v_like = format_ident!("{}Like", field_name);
    let v_not_like = format_ident!("{}NotLike", field_name);
    let v_is_null = format_ident!("{}IsNull", field_name);
    let v_is_not_null = format_ident!("{}IsNotNull", field_name);

    vec![
        quote! { #v_eq(String) },
        quote! { #v_neq(String) },
        quote! { #v_like(String) },
        quote! { #v_not_like(String) },
        quote! { #v_is_null },
        quote! { #v_is_not_null },
    ]
}

fn parse_boolean_operators(field_name: &str) -> Vec<proc_macro2::TokenStream> {
    let v_true = format_ident!("{}IsTrue", field_name);
    let v_false = format_ident!("{}IsFalse", field_name);

    vec![quote! { #v_true }, quote! { #v_false }]
}

fn parse_numeric_operators(field_name: &str, r#type: &syn::Type) -> Vec<proc_macro2::TokenStream> {
    let v_eq = format_ident!("{}Eq", field_name);
    let v_neq = format_ident!("{}Neq", field_name);
    let v_gt = format_ident!("{}Gt", field_name);
    let v_gte = format_ident!("{}Gte", field_name);
    let v_lt = format_ident!("{}Lt", field_name);
    let v_lte = format_ident!("{}Lte", field_name);
    let v_between = format_ident!("{}Between", field_name);
    let v_not_between = format_ident!("{}NotBetween", field_name);

    vec![
        quote! { #v_eq(#r#type) },
        quote! { #v_neq(#r#type) },
        quote! { #v_gt(#r#type) },
        quote! { #v_gte(#r#type) },
        quote! { #v_lt(#r#type) },
        quote! { #v_lte(#r#type) },
        quote! { #v_between(#r#type, #r#type) },
        quote! { #v_not_between(#r#type, #r#type) },
    ]
}

fn parse_uuid_or_scalar_operators(
    field_name: &str,
    r#type: &syn::Type,
) -> Vec<proc_macro2::TokenStream> {
    let v_eq = format_ident!("{}Eq", field_name);
    let v_neq = format_ident!("{}Neq", field_name);
    let v_is_null = format_ident!("{}IsNull", field_name);
    let v_is_not_null = format_ident!("{}IsNotNull", field_name);

    vec![
        quote! { #v_eq(#r#type) },
        quote! { #v_neq(#r#type) },
        quote! { #v_is_null },
        quote! { #v_is_not_null },
    ]
}

fn parse_datetime_operators(field_name: &str, r#type: &syn::Type) -> Vec<proc_macro2::TokenStream> {
    let v_on = format_ident!("{}On", field_name);
    let v_between = format_ident!("{}Between", field_name);
    let v_is_null = format_ident!("{}IsNull", field_name);
    let v_is_not_null = format_ident!("{}IsNotNull", field_name);

    vec![
        quote! { #v_on(#r#type) },
        quote! { #v_between(#r#type, #r#type) },
        quote! { #v_is_null },
        quote! { #v_is_not_null },
    ]
}

fn add_string_impls(
    sql: &mut Vec<proc_macro2::TokenStream>,
    bind: &mut Vec<proc_macro2::TokenStream>,
    col: &str,
    name: &str,
) {
    let eq = format_ident!("{}Eq", name);
    sql.push(quote! { Self::#eq(_) => { *idx+=1; format!("{} = ${}", #col, *idx) } });
    bind.push(quote! { Self::#eq(v) => q.bind(v) });

    let neq = format_ident!("{}Neq", name);
    sql.push(quote! { Self::#neq(_) => { *idx+=1; format!("{} <> ${}", #col, *idx) } });
    bind.push(quote! { Self::#neq(v) => q.bind(v) });

    let like = format_ident!("{}Like", name);
    sql.push(quote! { Self::#like(_) => { *idx+=1; format!("{} LIKE ${}", #col, *idx) } });
    bind.push(quote! { Self::#like(v) => q.bind(v) });

    let not_like = format_ident!("{}NotLike", name);
    sql.push(quote! { Self::#not_like(_) => { *idx+=1; format!("{} NOT LIKE ${}", #col, *idx) } });
    bind.push(quote! { Self::#not_like(v) => q.bind(v) });

    let is_null = format_ident!("{}IsNull", name);
    sql.push(quote! { Self::#is_null => format!("{} IS NULL", #col) });
    bind.push(quote! { Self::#is_null => q });

    let is_not_null = format_ident!("{}IsNotNull", name);
    sql.push(quote! { Self::#is_not_null => format!("{} IS NOT NULL", #col) });
    bind.push(quote! { Self::#is_not_null => q });
}

fn add_bool_impls(
    sql: &mut Vec<proc_macro2::TokenStream>,
    bind: &mut Vec<proc_macro2::TokenStream>,
    col: &str,
    name: &str,
) {
    let is_true = format_ident!("{}IsTrue", name);
    sql.push(quote! { Self::#is_true => format!("{} = TRUE", #col) });
    bind.push(quote! { Self::#is_true => q });

    let is_false = format_ident!("{}IsFalse", name);
    sql.push(quote! { Self::#is_false => format!("{} = FALSE", #col) });
    bind.push(quote! { Self::#is_false => q });
}

fn add_numeric_impls(
    sql: &mut Vec<proc_macro2::TokenStream>,
    bind: &mut Vec<proc_macro2::TokenStream>,
    col: &str,
    name: &str,
) {
    let ops = [
        ("Eq", "="),
        ("Neq", "<>"),
        ("Gt", ">"),
        ("Gte", ">="),
        ("Lt", "<"),
        ("Lte", "<="),
    ];
    for (suffix, op) in ops {
        let ident = format_ident!("{}{}", name, suffix);
        sql.push(quote! { Self::#ident(_) => { *idx+=1; format!("{} {} ${}", #col,#op,*idx) } });
        bind.push(quote! { Self::#ident(v) => q.bind(v) });
    }
    let between = format_ident!("{}Between", name);
    sql.push(quote! { Self::#between(_,_) => { *idx+=1; let a=*idx; *idx+=1; let b=*idx; format!("{} BETWEEN ${} AND ${}", #col,a,b) } });
    bind.push(quote! { Self::#between(v1,v2) => q.bind(v1).bind(v2) });

    let not_between = format_ident!("{}NotBetween", name);
    sql.push(quote! { Self::#not_between(_,_) => { *idx+=1; let a=*idx; *idx+=1; let b=*idx; format!("{} NOT BETWEEN ${} AND ${}", #col,a,b) } });
    bind.push(quote! { Self::#not_between(v1,v2) => q.bind(v1).bind(v2) });
}

fn add_uuid_impls(
    sql: &mut Vec<proc_macro2::TokenStream>,
    bind: &mut Vec<proc_macro2::TokenStream>,
    col: &str,
    name: &str,
) {
    let eq = format_ident!("{}Eq", name);
    sql.push(quote! { Self::#eq(_) => { *idx+=1; format!("{} = ${}", #col,*idx) } });
    bind.push(quote! { Self::#eq(v) => q.bind(v) });

    let neq = format_ident!("{}Neq", name);
    sql.push(quote! { Self::#neq(_) => { *idx+=1; format!("{} <> ${}", #col,*idx) } });
    bind.push(quote! { Self::#neq(v) => q.bind(v) });

    let is_null = format_ident!("{}IsNull", name);
    sql.push(quote! { Self::#is_null => format!("{} IS NULL", #col) });
    bind.push(quote! { Self::#is_null => q });

    let is_not_null = format_ident!("{}IsNotNull", name);
    sql.push(quote! { Self::#is_not_null => format!("{} IS NOT NULL", #col) });
    bind.push(quote! { Self::#is_not_null => q });
}

fn add_datetime_impls(
    sql: &mut Vec<proc_macro2::TokenStream>,
    bind: &mut Vec<proc_macro2::TokenStream>,
    col: &str,
    name: &str,
) {
    let on = format_ident!("{}On", name);
    sql.push(quote! { Self::#on(_) => { *idx+=1; format!("{} = ${}", #col,*idx) } });
    bind.push(quote! { Self::#on(v) => q.bind(v) });

    let between = format_ident!("{}Between", name);
    sql.push(quote! { Self::#between(_,_) => { *idx+=1; let a=*idx; *idx+=1; let b=*idx; format!("{} BETWEEN ${} AND ${}", #col,a,b) } });
    bind.push(quote! { Self::#between(v1,v2) => q.bind(v1).bind(v2) });

    let is_null = format_ident!("{}IsNull", name);
    sql.push(quote! { Self::#is_null => format!("{} IS NULL", #col) });
    bind.push(quote! { Self::#is_null => q });

    let is_not_null = format_ident!("{}IsNotNull", name);
    sql.push(quote! { Self::#is_not_null => format!("{} IS NOT NULL", #col) });
    bind.push(quote! { Self::#is_not_null => q });
}
