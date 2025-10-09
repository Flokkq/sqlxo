// filter_macros/src/lib.rs
#![feature(let_chains)]

use heck::{ToPascalCase, ToSnakeCase};
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, spanned::Spanned, Data, DeriveInput, Error, Fields, Ident, Lit, Meta,
    NestedMeta, Visibility,
};

#[proc_macro_derive(Query, attributes(filter, primary_key, foreign_key))]
pub fn derive_query(input: TokenStream) -> TokenStream {
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

    let struct_ident = &input.ident;
    let query_ident = format_ident!("{}Query", struct_ident);
    let sort_ident = format_ident!("{}Sort", struct_ident);
    let join_ident = format_ident!("{}Join", struct_ident);

    let data = match &input.data {
        Data::Struct(s) => s,
        _ => {
            return Error::new_spanned(&input.ident, "`Query` derive only supports structs")
                .to_compile_error()
                .into()
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

    let mut query_variants = Vec::new();
    let mut sort_variants = Vec::new();
    let mut write_arms = Vec::new();
    let mut sort_sql_arms = Vec::new();

    // JOIN metadata aus Feld-Attributen
    let mut pk_field: Option<String> = None;
    let mut fks: Vec<FkSpec> = Vec::new();

    for field in fields.iter() {
        let field_ident = field.ident.clone().unwrap();
        let field_name_pascal = field_ident.to_string().to_pascal_case();
        let field_name_snake = field_ident.to_string().to_snake_case();
        let ty = &field.ty;

        // PK / FK-Attribute
        for attr in &field.attrs {
            if attr.path.is_ident("primary_key") {
                if pk_field.is_some() {
                    return Error::new(attr.span(), "duplicate #[primary_key] on struct")
                        .to_compile_error()
                        .into();
                }
                pk_field = Some(field_name_snake.clone());
            }
            if attr.path.is_ident("foreign_key") {
                // #[foreign_key(to = "table.pk")]
                let meta = attr
                    .parse_meta()
                    .map_err(|_| Error::new_spanned(attr, "invalid #[foreign_key] attribute"))
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
                        NestedMeta::Meta(Meta::NameValue(nv)) if nv.path.is_ident("to") => {
                            match nv.lit {
                                Lit::Str(ref s) => {
                                    if to_value.is_some() {
                                        return Error::new_spanned(
                                            nv,
                                            "duplicate key `to` in #[foreign_key]",
                                        )
                                        .to_compile_error()
                                        .into();
                                    }
                                    to_value = Some(s.value());
                                }
                                other => {
                                    return Error::new_spanned(
                                        other,
                                        r#"expected string literal: to = "table.pk""#,
                                    )
                                    .to_compile_error()
                                    .into()
                                }
                            }
                        }
                        other => return Error::new_spanned(
                            other,
                            r#"unknown key in #[foreign_key]; only `to = "table.pk"` is supported"#,
                        )
                        .to_compile_error()
                        .into(),
                    }
                }
                let to = to_value.expect(r#"missing `to = "table.pk"` in #[foreign_key]"#);
                let mut parts = to.split('.');
                let right_table = parts
                    .next()
                    .ok_or_else(|| Error::new(attr.span(), "missing right table in `to`"))
                    .unwrap()
                    .to_string();
                let right_pk = parts
                    .next()
                    .ok_or_else(|| Error::new(attr.span(), "missing right pk in `to`"))
                    .unwrap()
                    .to_string();
                if parts.next().is_some() {
                    return Error::new(
                        attr.span(),
                        r#"invalid `to` — expected "table.pk" with exactly one dot"#,
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

        // bestehende Query/Sort-Generierung
        let asc = format_ident!("By{}Asc", field_name_pascal);
        let desc = format_ident!("By{}Desc", field_name_pascal);
        sort_variants.push(quote! { #asc });
        sort_variants.push(quote! { #desc });

        let col: &str = &field_name_snake;
        sort_sql_arms.push(quote! { Self::#asc  => format!("{} ASC",  #col) });
        sort_sql_arms.push(quote! { Self::#desc => format!("{} DESC", #col) });

        match classify_type(ty) {
            Kind::String => {
                query_variants.extend(parse_string_operators(&field_name_pascal));
                add_string_write_arms(&mut write_arms, &field_name_snake, &field_name_pascal);
            }
            Kind::Bool => {
                query_variants.extend(parse_boolean_operators(&field_name_pascal));
                add_bool_write_arms(&mut write_arms, &field_name_snake, &field_name_pascal);
            }
            Kind::Number => {
                query_variants.extend(parse_numeric_operators(&field_name_pascal, ty));
                add_numeric_write_arms(&mut write_arms, &field_name_snake, &field_name_pascal);
            }
            Kind::UuidOrScalarEq => {
                query_variants.extend(parse_uuid_or_scalar_operators(&field_name_pascal, ty));
                add_uuid_write_arms(&mut write_arms, &field_name_snake, &field_name_pascal);
            }
            Kind::DateTime => {
                query_variants.extend(parse_datetime_operators(&field_name_pascal, ty));
                add_datetime_write_arms(&mut write_arms, &field_name_snake, &field_name_pascal);
            }
        }
    }

    // Join-Enum + Arms (robust wenn fks.is_empty())
    let (join_variants, join_to_sql_arms, join_kind_arms) =
        build_join_codegen(struct_ident, &table_name, &fks);

    let expanded = quote! {
        impl filter_traits::QueryContext for #struct_ident {
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

        // NEW: aus #[foreign_key] generiert (oder Sentinel, wenn keine vorhanden)
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum #join_ident {
            #(#join_variants),*
        }

        // NEW: Join glue
        impl filter_traits::SqlJoin for #join_ident {
            fn to_sql(&self) -> String {
                match self {
                    #(#join_to_sql_arms),*
                }
            }
            fn kind(&self) -> filter_traits::JoinKind {
                match self {
                    #(#join_kind_arms),*
                }
            }
        }

        impl filter_traits::Filterable for #query_ident {
            type Entity = #struct_ident;

            fn write<W: filter_traits::SqlWrite>(&self, w: &mut W) {
                match self {
                    #(#write_arms),*
                }
            }
        }

        impl filter_traits::Sortable for #sort_ident {
            type Entity = #struct_ident;

            fn sort_clause(&self) -> String {
                match self {
                    #(#sort_sql_arms),*
                }
            }
        }
    };

    expanded.into()
}

struct FkSpec {
    fk_field_snake: String,
    fk_field_pascal: Ident,
    right_table: String,
    right_pk: String,
}

fn build_join_codegen(
    left_struct: &Ident,
    left_table: &str,
    fks: &[FkSpec],
) -> (
    Vec<proc_macro2::TokenStream>, // enum variants
    Vec<proc_macro2::TokenStream>, // to_sql arms
    Vec<proc_macro2::TokenStream>, // kind arms
) {
    let mut variants = Vec::new();
    let mut to_sql_arms = Vec::new();
    let mut kind_arms = Vec::new();

    if fks.is_empty() {
        let never_var = format_ident!("__Never");
        variants.push(quote! { #never_var(::core::convert::Infallible) });
        to_sql_arms.push(quote! { Self::#never_var(_) => unreachable!("no joins for this model") });
        kind_arms.push(quote! { Self::#never_var(_) => unreachable!("no joins for this model") });
        return (variants, to_sql_arms, kind_arms);
    }

    for fk in fks {
        let right_pascal = fk.right_table.to_pascal_case();
        let var = format_ident!("{}To{}By{}", left_struct, right_pascal, fk.fk_field_pascal);

        variants.push(quote! { #var(filter_traits::JoinKind) });

        let right_table = fk.right_table.clone();
        let on_left = format!(r#""{}"."{}""#, left_table, fk.fk_field_snake);
        let on_right = format!(r#""{}"."{}""#, right_table, fk.right_pk);

        to_sql_arms.push(quote! {
            Self::#var(kind) => match kind {
                filter_traits::JoinKind::Inner =>
                    format!(r#" INNER JOIN {} ON {} = {}"#, #right_table, #on_left, #on_right),
                filter_traits::JoinKind::Left  =>
                    format!(r#" LEFT JOIN {} ON {} = {}"#,  #right_table, #on_left, #on_right),
            }
        });
        kind_arms.push(quote! { Self::#var(k) => *k });
    }

    (variants, to_sql_arms, kind_arms)
}

/// Extracts `table_name` from `#[filter(table_name = "...")]`.
fn extract_table_name(input: &DeriveInput) -> syn::Result<Option<String>> {
    let mut value: Option<String> = None;

    for attr in &input.attrs {
        if !attr.path.is_ident("filter") {
            continue;
        }

        let meta = attr
            .parse_meta()
            .map_err(|_| syn::Error::new_spanned(attr, "invalid #[filter] attribute"))?;

        let list = match meta {
            Meta::List(list) => list,
            _ => {
                return Err(syn::Error::new_spanned(
                    attr,
                    "expected #[filter(table_name = \"...\")]",
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

    Ok(value)
}

enum Kind {
    String,
    Bool,
    Number,
    UuidOrScalarEq,
    DateTime,
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

fn add_string_write_arms(write_arms: &mut Vec<proc_macro2::TokenStream>, col: &str, name: &str) {
    let eq = format_ident!("{}Eq", name);
    write_arms
        .push(quote! { Self::#eq(v) => { w.push(concat!(#col, " = ")); w.bind(v.clone()); } });

    let neq = format_ident!("{}Neq", name);
    write_arms
        .push(quote! { Self::#neq(v) => { w.push(concat!(#col, " <> ")); w.bind(v.clone()); } });

    let like = format_ident!("{}Like", name);
    write_arms
        .push(quote! { Self::#like(v) => { w.push(concat!(#col, " LIKE ")); w.bind(v.clone()); } });

    let not_like = format_ident!("{}NotLike", name);
    write_arms.push(quote! { Self::#not_like(v) => { w.push(concat!(#col, " NOT LIKE ")); w.bind(v.clone()); } });

    let is_null = format_ident!("{}IsNull", name);
    write_arms.push(quote! { Self::#is_null => { w.push(concat!(#col, " IS NULL")); } });

    let is_not_null = format_ident!("{}IsNotNull", name);
    write_arms.push(quote! { Self::#is_not_null => { w.push(concat!(#col, " IS NOT NULL")); } });
}

fn add_bool_write_arms(write_arms: &mut Vec<proc_macro2::TokenStream>, col: &str, name: &str) {
    let is_true = format_ident!("{}IsTrue", name);
    write_arms.push(quote! { Self::#is_true => { w.push(concat!(#col, " = TRUE")); } });

    let is_false = format_ident!("{}IsFalse", name);
    write_arms.push(quote! { Self::#is_false => { w.push(concat!(#col, " = FALSE")); } });
}

fn add_numeric_write_arms(write_arms: &mut Vec<proc_macro2::TokenStream>, col: &str, name: &str) {
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
        write_arms.push(quote! {
            Self::#ident(v) => { w.push(concat!(#col, " ", #op, " ")); w.bind(*v); }
        });
    }
    let between = format_ident!("{}Between", name);
    write_arms.push(quote! {
        Self::#between(a,b) => {
            w.push(concat!(#col, " BETWEEN "));
            w.bind(*a);
            w.push(" AND ");
            w.bind(*b);
        }
    });

    let not_between = format_ident!("{}NotBetween", name);
    write_arms.push(quote! {
        Self::#not_between(a,b) => {
            w.push(concat!(#col, " NOT BETWEEN "));
            w.bind(*a);
            w.push(" AND ");
            w.bind(*b);
        }
    });
}

fn add_uuid_write_arms(write_arms: &mut Vec<proc_macro2::TokenStream>, col: &str, name: &str) {
    let eq = format_ident!("{}Eq", name);
    write_arms.push(quote! { Self::#eq(v) => { w.push(concat!(#col, " = ")); w.bind(*v); } });

    let neq = format_ident!("{}Neq", name);
    write_arms.push(quote! { Self::#neq(v) => { w.push(concat!(#col, " <> ")); w.bind(*v); } });

    let is_null = format_ident!("{}IsNull", name);
    write_arms.push(quote! { Self::#is_null => { w.push(concat!(#col, " IS NULL")); } });

    let is_not_null = format_ident!("{}IsNotNull", name);
    write_arms.push(quote! { Self::#is_not_null => { w.push(concat!(#col, " IS NOT NULL")); } });
}

fn add_datetime_write_arms(write_arms: &mut Vec<proc_macro2::TokenStream>, col: &str, name: &str) {
    let on = format_ident!("{}On", name);
    write_arms.push(quote! { Self::#on(v) => { w.push(concat!(#col, " = ")); w.bind(*v); } });

    let between = format_ident!("{}Between", name);
    write_arms.push(quote! {
        Self::#between(a,b) => {
            w.push(concat!(#col, " BETWEEN "));
            w.bind(*a);
            w.push(" AND ");
            w.bind(*b);
        }
    });

    let is_null = format_ident!("{}IsNull", name);
    write_arms.push(quote! { Self::#is_null => { w.push(concat!(#col, " IS NULL")); } });

    let is_not_null = format_ident!("{}IsNotNull", name);
    write_arms.push(quote! { Self::#is_not_null => { w.push(concat!(#col, " IS NOT NULL")); } });
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

    // module name ctx_a_b_c
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

    let expanded = quote! {
        pub mod #mod_name {
            pub struct Ctx;

            impl filter_traits::QueryContext for Ctx {
                const TABLE: &'static str = <#base_ty as filter_traits::QueryContext>::TABLE;

                type Model = <#base_ty as filter_traits::QueryContext>::Model;
                type Query = <#base_ty as filter_traits::QueryContext>::Query;
                type Sort  = <#base_ty as filter_traits::QueryContext>::Sort;
                type Join  = <#base_ty as filter_traits::QueryContext>::Join;
            }

            // Re-exports
            pub type Where = <#base_ty as filter_traits::QueryContext>::Query;
            pub type Sort  = <#base_ty as filter_traits::QueryContext>::Sort;
            pub type Join  = <#base_ty as filter_traits::QueryContext>::Join;
            pub use filter_traits::JoinKind;
        }
    };
    expanded.into()
}
