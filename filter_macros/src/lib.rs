#![feature(let_chains)]

use heck::ToSnakeCase;
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::{fmt, str::FromStr};
use syn::{parse_macro_input, Data, DeriveInput, Fields, Lit, LitStr, Meta, NestedMeta};

const SUFFIXES: [&str; 16] = [
    "Neq",
    "NotBetween",
    "NotIn",
    "NotLike",
    "NotNull",
    "Above",
    "Below",
    "Between",
    "Eq",
    "Gt",
    "Gte",
    "In",
    "IsNull",
    "Like",
    "Lt",
    "Lte",
];

fn infer_column(var: &str) -> String {
    for suf in &SUFFIXES {
        if let Some(base) = var.strip_suffix(suf) {
            return base.to_snake_case();
        }
    }
    var.to_snake_case()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Operator {
    Eq,
    Neq,
    Gt,
    Gte,
    Lt,
    Lte,
    Like,
    NotLike,
    In,
    NotIn,
    Between,
    NotBetween,
    IsNull,
    NotNull,
}

impl FromStr for Operator {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "eq" => Ok(Self::Eq),
            "neq" => Ok(Self::Neq),
            "gt" => Ok(Self::Gt),
            "gte" => Ok(Self::Gte),
            "lt" => Ok(Self::Lt),
            "lte" => Ok(Self::Lte),
            "like" => Ok(Self::Like),
            "not_like" => Ok(Self::NotLike),
            "in" => Ok(Self::In),
            "not_in" => Ok(Self::NotIn),
            "between" => Ok(Self::Between),
            "not_between" => Ok(Self::NotBetween),
            "is_null" => Ok(Self::IsNull),
            "not_null" => Ok(Self::NotNull),
            other => Err(format!(
                "unknown operator `{}`; allowed = eq, neq, gt, gte, lt, lte, like, not_like, in, not_in, between, not_between, is_null, not_null",
                other
            )),
        }
    }
}

impl fmt::Display for Operator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sql = match self {
            Self::Eq => "=",
            Self::Neq => "<>",
            Self::Gt => ">",
            Self::Gte => ">=",
            Self::Lt => "<",
            Self::Lte => "<=",
            Self::Like => "LIKE",
            Self::NotLike => "NOT LIKE",
            Self::In => "IN",
            Self::NotIn => "NOT IN",
            Self::Between => "BETWEEN",
            Self::NotBetween => "NOT BETWEEN",
            Self::IsNull => "IS NULL",
            Self::NotNull => "IS NOT NULL",
        };
        write!(f, "{sql}")
    }
}

const INFER_DEFAULTS: bool = cfg!(feature = "infer-defaults");

#[proc_macro_derive(Filter, attributes(filter, filter_config))]
pub fn derive_filter(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let enum_name = input.ident.clone();

    let mut require_attrs = false;
    for attr in &input.attrs {
        if let Ok(Meta::List(meta)) = attr.parse_meta() {
            if meta.path.is_ident("filter_config") {
                for nm in meta.nested.iter() {
                    if let NestedMeta::Meta(Meta::Path(p)) = nm {
                        if p.is_ident("require_attrs") {
                            require_attrs = true;
                        }
                    }
                }
            }
        }
    }

    let mut table = None::<String>;
    let mut entity = None::<syn::Ident>;
    for attr in &input.attrs {
        if let Ok(Meta::List(meta)) = attr.parse_meta() {
            if meta.path.is_ident("filter") {
                for nm in meta.nested.iter() {
                    if let NestedMeta::Meta(Meta::NameValue(nv)) = nm {
                        if nv.path.is_ident("table") {
                            if let Lit::Str(s) = &nv.lit {
                                table = Some(s.value());
                            }
                        } else if nv.path.is_ident("entity") {
                            if let Lit::Str(s) = &nv.lit {
                                entity = Some(syn::Ident::new(&s.value(), s.span()));
                            }
                        }
                    }
                }
            }
        }
    }
    let table = table.expect("missing #[filter(table=\"...\")] on enum");
    let entity = entity.expect("missing #[filter(entity=\"...\")] on enum");

    let Data::Enum(data_enum) = &input.data else {
        panic!("`Filter` can only be derived on enums")
    };

    let mut clause_arms = Vec::new();
    let mut bind_arms = Vec::new();

    for var in &data_enum.variants {
        let ident = &var.ident;
        let ident_str = ident.to_string();
        let fields = &var.fields;

        let field_count = match fields {
            Fields::Unit => 0,
            Fields::Unnamed(u) if u.unnamed.len() == 1 => 1,
            _ => panic!("variant {ident} must be unit or single-field tuple"),
        };

        let mut col_override = None::<String>;
        let mut op_override = None::<String>;

        for attr in &var.attrs {
            if let Ok(Meta::List(meta)) = attr.parse_meta() {
                if meta.path.is_ident("filter") {
                    for nm in meta.nested {
                        if let NestedMeta::Meta(Meta::NameValue(nv)) = nm {
                            if nv.path.is_ident("name") {
                                if let Lit::Str(s) = &nv.lit {
                                    col_override = Some(s.value());
                                }
                            } else if nv.path.is_ident("op") {
                                if let Lit::Str(s) = &nv.lit {
                                    op_override = Some(s.value());
                                }
                            }
                        }
                    }
                }
            }
        }

        let col = col_override.unwrap_or_else(|| {
            if require_attrs && !INFER_DEFAULTS {
                panic!("variant `{ident}` missing #[filter(name=\"...\", op=\"...\")]");
            }
            infer_column(&ident_str)
        });

        let op_enum: Operator = if let Some(key) = op_override {
            key.parse()
                .unwrap_or_else(|e| panic!("invalid op on {ident}: {e}"))
        } else {
            match SUFFIXES
                .iter()
                .find(|s| ident_str.ends_with(*s))
                .map(|s| s.to_snake_case())
                .as_deref()
            {
                Some("above") => Operator::Gt,
                Some("below") => Operator::Lt,
                Some(k) => k.parse().unwrap(),
                None => Operator::Eq,
            }
        };

        let col_lit = LitStr::new(&col, Span::call_site());
        let op_lit = LitStr::new(&op_enum.to_string(), Span::call_site());

        if field_count == 0 {
            clause_arms.push(quote! { Self::#ident => format!("{} {}", #col_lit, #op_lit) });
            bind_arms.push(quote! { Self::#ident => q });
        } else {
            clause_arms
                .push(quote! { Self::#ident(_) => format!("{} {} ${}", #col_lit, #op_lit, idx) });
            bind_arms.push(quote! { Self::#ident(val) => q.bind(val) });
        }
    }

    let out = quote! {
            impl filter_traits::Filterable for #enum_name {
                type Entity = #entity;

                fn table_name() -> &'static str { #table }

                fn filter_clause(&self, idx: usize) -> String {
                    match self { #(#clause_arms),* }
                }

                fn bind<'q>(
                    self,
                    q: sqlx::query::QueryAs<
                        'q,
                        sqlx::Postgres,
                        <Self as filter_traits::Filterable>::Entity,
                        sqlx::postgres::PgArguments
                    >
                ) -> sqlx::query::QueryAs<
                    'q,
                    sqlx::Postgres,
                    <Self as filter_traits::Filterable>::Entity,
                    sqlx::postgres::PgArguments
                > {
                    match self { #(#bind_arms),* }
                }
            }

    impl #enum_name {
        pub async fn filter_all(
            pool: &sqlx::PgPool,
            filters: Vec<Self>,
        ) -> Result<
            Vec<<Self as filter_traits::Filterable>::Entity>,
            anyhow::Error
        > {
            let sql = Self::to_sql(&filters);
            let mut q = sqlx::query_as::<_, <Self as filter_traits::Filterable>::Entity>(&sql);
            let q = filters.into_iter().fold(q, |qq, f| f.bind(qq));
            q.fetch_all(pool).await.map_err(Into::into)
        }

        #[cfg(test)]
        pub fn to_sql(filters: &[Self]) -> String {
            let where_clause = filters
                .iter()
                .enumerate()
                .map(|(i, f)| f.filter_clause(i + 1))
                .collect::<Vec<_>>()
                .join(" AND ");

            if where_clause.is_empty() {
                format!("SELECT * FROM {}", Self::table_name())
            } else {
                format!("SELECT * FROM {} WHERE {}", Self::table_name(), where_clause)
            }
        }
    }
        };
    TokenStream::from(out)
}

#[cfg(test)]
mod tests {
    use super::{infer_column, Operator, SUFFIXES};

    #[test]
    fn infer_no_suffix() {
        assert_eq!(infer_column("FooBar"), "foo_bar");
    }

    #[test]
    fn infer_every_suffix() {
        for &s in &SUFFIXES {
            assert_eq!(infer_column(&format!("X{}", s)), "x", "suffix {s}");
        }
    }

    #[test]
    fn operator_parse_display_roundtrip() {
        for key in [
            "eq",
            "neq",
            "gt",
            "gte",
            "lt",
            "lte",
            "like",
            "not_like",
            "in",
            "not_in",
            "between",
            "not_between",
            "is_null",
            "not_null",
        ] {
            let op: Operator = key.parse().unwrap();
            assert!(!op.to_string().is_empty());
        }
    }
}
