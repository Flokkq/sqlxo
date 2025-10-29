#![cfg(test)]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlo_macros::{Query, WebQuery};
use sqlo_traits::{DtoFilter, JoinKind};
use sqlx::FromRow;
use uuid::Uuid;

use crate::builder::{QueryBuilder, QueryPlan};
use crate::expression::Expression;
use crate::head::BuildType;
use crate::pagination::Pagination;
use crate::{and, or, order_by};

pub trait NormalizeString {
    fn normalize(&self) -> String;
}

impl NormalizeString for String {
    fn normalize(&self) -> String {
        self.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

impl NormalizeString for &str {
    fn normalize(&self) -> String {
        self.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

#[allow(dead_code)]
#[derive(Debug, FromRow, Clone, Query, PartialEq)]
pub struct Item {
    #[primary_key]
    id: Uuid,
    name: String,
    description: String,
    price: f32,
    amount: i32,
    active: bool,
    due_date: sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,

    #[foreign_key(to = "material.id")]
    material_id: Option<Uuid>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, WebQuery, Deserialize, Serialize)]
pub struct ItemDto {
    id: Uuid,
    name: String,
    description: String,
    price: f32,
    amount: i32,
    active: bool,
    due_date: sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,
}

#[allow(dead_code)]
#[derive(Debug, FromRow, Clone, Query)]
pub struct Material {
    #[primary_key]
    id: Uuid,

    name: String,
    long_name: String,
    description: String,
}

#[test]
fn expression_macros() {
    let plain_query = Expression::Or(vec![
        Expression::And(vec![
            Expression::Leaf(ItemQuery::NameLike("%SternLampe%".into())),
            Expression::Leaf(ItemQuery::DescriptionNeq("Hohlweg".into())),
        ]),
        Expression::Leaf(ItemQuery::PriceGt(1800f32)),
    ]);

    let long_macro_query = or![
        and![
            Expression::Leaf(ItemQuery::NameLike("%SternLampe%".into())),
            Expression::Leaf(ItemQuery::DescriptionNeq("Hohlweg".into())),
        ],
        Expression::Leaf(ItemQuery::PriceGt(1800f32)),
    ];

    let short_macro_query = or![
        and![
            ItemQuery::NameLike("%SternLampe%".into()),
            ItemQuery::DescriptionNeq("Hohlweg".into()),
        ],
        ItemQuery::PriceGt(1800f32),
    ];

    assert_eq!(plain_query, long_macro_query);
    assert_eq!(long_macro_query, short_macro_query);
}

#[test]
fn sort_macros() {
    let plain_sort = crate::sort::SortOrder(vec![ItemSort::ByAmountAsc, ItemSort::ByNameDesc]);
    let short_macro_sort = order_by![ItemSort::ByAmountAsc, ItemSort::ByNameDesc];

    assert_eq!(plain_sort, short_macro_sort);
}

#[test]
fn query_builder() {
    let plan: QueryPlan<Item> = QueryBuilder::from_ctx()
        .join(ItemJoin::ItemToMaterialByMaterialId(JoinKind::Left))
        .r#where(and![
            ItemQuery::NameLike("Clemens".into()),
            or![ItemQuery::PriceGt(1800.00f32), ItemQuery::DescriptionIsNull,]
        ])
        .order_by(order_by![ItemSort::ByNameAsc, ItemSort::ByPriceDesc])
        .paginate(Pagination {
            page: 2,
            page_size: 50,
        })
        .build();

    assert_eq!(
        plan.sql(BuildType::Raw).trim_start(),
        r#"
            LEFT JOIN material ON "item"."material_id" = "material"."id"
            WHERE (name LIKE $1 AND (price > $2 OR description IS NULL))
            ORDER BY name ASC, price DESC
            LIMIT $3 OFFSET $4
        "#
        .normalize()
    )
}

#[test]
fn deserialize_itemdto_sqlo_json() {
    let json: Value = json!({
        "filter": {
            "and": [
                { "name": { "like": "%Sternlampe%" } },
                { "or": [
                    { "price": { "gt": 18.00 } },
                    { "description": { "neq": "von Hohlweg" } }
                ]}
            ]
        },
        "sort": [
            { "name": "asc" },
            { "description": "desc" }
        ],
        "page": { "pageSize": 10, "pageNo": 1 }
    });

    let f: DtoFilter<ItemDto> = serde_json::from_value(json).expect("valid ItemDtoFilter");

    assert_eq!(f.page.page_size, 10);
    assert_eq!(f.page.page_no, 1);
}
