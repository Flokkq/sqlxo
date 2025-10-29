# sqlxo

[![Crates.io](https://img.shields.io/crates/v/sqlxo.svg)](https://crates.io/crates/sqlxo)

Type-safe SQL query building on top of `sqlx`, driven by auto-generated enums from the `Query` derive macro.

## Installation

```toml
# Cargo.toml
[dependencies]
sqlxo = "0.1.1"
````

## Example

The `Query` derive macro generates all querying variants and sort fields for your model.

```rust
use sqlxo::{Query, QueryBuilder, Pagination, and, or, order_by};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

#[derive(Debug, FromRow, Clone, Query, PartialEq)]
pub struct Item {
    #[primary_key]
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub price: f32,
    pub amount: i32,
    pub active: bool,
    pub due_date: chrono::DateTime<chrono::Utc>,

    #[foreign_key(to = "material.id")]
    pub material_id: Option<Uuid>,
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let pool: PgPool = PgPool::connect("postgres://...").await?;

    let maybe: Option<Item> = QueryBuilder::<Item>::from_ctx()
        .r#where(and![
            ItemQuery::NameEq("test".into()),
            or![
                ItemQuery::PriceLt(10.00f32),
                ItemQuery::AmountEq(2)
            ]
        ])
        .order_by(order_by![
            ItemSort::ByNameAsc,
            ItemSort::ByPriceDesc
        ])
        .paginate(Pagination { page: 0, page_size: 50 })
        .build()
        .fetch_optional(&pool)
        .await?;

    println!("{maybe:?}");
    Ok(())
}
```

This builds and executes

```sql
SELECT *
FROM item
WHERE (name = $1 AND (price < $2 OR amount = $3))
ORDER BY name ASC, price DESC
LIMIT $4 OFFSET $5
```

## Support

postgres only

## Contributing

Bug fixes are welcome. For features, please open an issue first.
