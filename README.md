# sqlxo

[![Crates.io](https://img.shields.io/crates/v/sqlxo.svg)](https://crates.io/crates/sqlxo)

Type-safe SQL query building on top of `sqlx`, driven by auto-generated enums from the `Query` derive macro.

> [!IMPORTANT]
> This crate is still under development. Currently `sqlxo` does not follow SemVer and can have broken releases or skip versions, due to trouble in the release workflow.
> Once `sqlxo` hits v1.0.0 it will be considered stable and follow SemVer.

sqlxo supports basic features of an ORM and RESTful queries that get converted into database queries. Both features are still early in development and lack important features
- aggregations
- joins
- permissions
- caching
- currently only supports READ operations

## Examples

### ORM

The `Query` derive macro generates all querying variants and sort fields for your model.

<details>
    <summary>Click to expand</summary>

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

</details>

### RESTful queries


The `WebQuery` derive macro in combination with the `#[bind]` attribute generates all querying variants and sort fields with mapping for your model. `sqlxo` ensures that only operations and queries the domain supports are serialized and sent to the db.

<details>
    <summary>Click to expand</summary>

```rust
#[derive(Debug, FromRow, Clone, Query, PartialEq)]
pub struct Item {
	#[primary_key]
	id:          Uuid,
	name:        String,
	price:       f32,
}

#[allow(dead_code)]
#[bind(Item)]
#[derive(Debug, Clone, WebQuery, Deserialize, Serialize)]
pub struct ItemDto {
	id:             Uuid,
	#[sqlxo(field = "name")]
	different_name: String,
	price:          f32,
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    let pool: PgPool = PgPool::connect("postgres://...").await?;

	let json: value = json!({ "filter": {
			"and": [
				{ "id": { "eq": "ce3cd6cc-66a7-4c72-9ad5-6e3dbae2fa02" } },
				{ "or": [
					{ "price": { "gt": 18.00 } },
					{ "different_name": { "like": "%bolt%" } }
				]}
			]
		},
		"sort": [
			{ "different_name": "asc" },
			{ "price": "desc" }
		],
		"page": { "pageSize": 10, "pageNo": 1 }
	});

	let filter: DtoFilter<ItemDto> = serde_json::from_value(json).unwrap();

	let items: Vec<Item> = 
        QueryBuilder::<Item>::from_dto::<ItemDto>(&filter)
		.build()
        .fetch_all(&pool)
        .await?;

    println!("Found {} items", items.len());
}
```

</details>

## Support

postgres is the only supported database. After sqlxo is stable more databases supported by slqx may follow. 

## Contributing

Bug fixes are welcome. For features, please open an issue first.
