use crate::helpers::{
	HardDeleteItem,
	HardDeleteItemQuery,
	SoftDeleteItem,
	SoftDeleteItemQuery,
};
use claims::assert_some_eq;
use sqlx::migrate;
use sqlx::postgres::PgConnectOptions;
use sqlx::postgres::PgPoolOptions;
use sqlx::postgres::PgSslMode;
use sqlx::PgPool;
use sqlxo::and;
use sqlxo::blocks::BuildableFilter;
use sqlxo::blocks::BuildablePage;
use sqlxo::blocks::BuildableSort;
use sqlxo::blocks::Expression;
use sqlxo::blocks::Page;
use sqlxo::blocks::Pagination;
use sqlxo::or;
use sqlxo::order_by;
use sqlxo::Buildable;
use sqlxo::ExecutablePlan;
use sqlxo::FetchablePlan;
use sqlxo::QueryBuilder;
use uuid::Uuid;

use crate::helpers::Item;
use crate::helpers::ItemQuery;
use crate::helpers::ItemSort;

#[derive(Debug, Clone)]
pub struct DatabaseSettings {
	pub username:      String,
	pub password:      String,
	pub port:          u16,
	pub host:          String,
	pub database_name: String,
	pub require_ssl:   bool,
}

impl DatabaseSettings {
	pub fn with_db(&self) -> PgConnectOptions {
		let options = self.without_db().database(&self.database_name);
		options
	}

	pub fn without_db(&self) -> PgConnectOptions {
		let pg_ssl_mode = if self.require_ssl {
			PgSslMode::Require
		} else {
			PgSslMode::Prefer
		};
		let ssl_mode = pg_ssl_mode;

		PgConnectOptions::new()
			.host(&self.host)
			.username(&self.username)
			.password(&self.password)
			.port(self.port)
			.ssl_mode(ssl_mode)
	}
}

pub async fn get_connection_pool() -> PgPool {
	let mut cfg = DatabaseSettings {
		username:      "postgres".into(),
		password:      "password".into(),
		port:          2345,
		host:          "localhost".into(),
		database_name: "postgres".into(),
		require_ssl:   false,
	};

	let server_pool = PgPoolOptions::new()
		.max_connections(1)
		.connect_with(cfg.clone().without_db())
		.await
		.expect("connect server");

	let db_name = Uuid::new_v4().to_string();

	sqlx::query(&format!(r#"CREATE DATABASE "{}""#, db_name))
		.execute(&server_pool)
		.await
		.expect("create db");

	cfg.database_name = db_name.clone();
	let pool = PgPoolOptions::new()
		.max_connections(5)
		.connect_with(cfg.with_db())
		.await
		.expect("connect new db");

	migrate!("../migrations").run(&pool).await.unwrap();

	pool
}

async fn insert_item(item: &Item, pool: &PgPool) -> Result<(), sqlx::Error> {
	sqlx::query(
        r#"
            INSERT INTO item (
                id, name, description, price, amount, active, due_date, material_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
    )
    .bind(item.id)
    .bind(&item.name)
    .bind(&item.description)
    .bind(item.price)
    .bind(item.amount)
    .bind(item.active)
    .bind(item.due_date)
    .bind(item.material_id)
    .execute(pool)
    .await.map(|_| ())
}

#[tokio::test]
async fn query_returns_expected_values() {
	let pool = get_connection_pool().await;
	let item = Item::default();

	insert_item(&item, &pool).await.unwrap();

	let maybe: Option<Item> = QueryBuilder::<Item>::read()
		.r#where(and![ItemQuery::NameEq("test".into()), or![
			ItemQuery::PriceLt(10.00f32),
			ItemQuery::AmountEq(2)
		]])
		.order_by(order_by![ItemSort::ByNameAsc, ItemSort::ByPriceDesc])
		.paginate(Pagination {
			page:      0,
			page_size: 50,
		})
		.build()
		.fetch_optional(&pool)
		.await
		.unwrap();

	assert_some_eq!(maybe, item);
}

#[tokio::test]
async fn query_returns_page() {
	let pool = get_connection_pool().await;
	let item = Item::default();

	insert_item(&item, &pool).await.unwrap();

	let page: Page<Item> = QueryBuilder::<Item>::read()
		.r#where(Expression::Leaf(ItemQuery::NameEq("test".into())))
		.paginate(Pagination {
			page:      0,
			page_size: 50,
		})
		.build()
		.fetch_page(&pool)
		.await
		.unwrap();

	assert_eq!(page.total, 1);
	assert_eq!(page.page, 0);
	assert_eq!(page.page_size, 50);
	assert_eq!(page.items, vec![item]);
}

#[tokio::test]
async fn query_exists() {
	let pool = get_connection_pool().await;
	let item = Item::default();

	insert_item(&item, &pool).await.unwrap();

	let exists: bool = QueryBuilder::<Item>::read()
		.r#where(Expression::Leaf(ItemQuery::NameEq("test".into())))
		.build()
		.exists(&pool)
		.await
		.unwrap();

	assert!(exists);
}

async fn insert_hard_delete_item(
	item: &HardDeleteItem,
	pool: &PgPool,
) -> Result<(), sqlx::Error> {
	sqlx::query(
		r#"
		INSERT INTO hard_delete_item (id, name, description, price)
		VALUES ($1, $2, $3, $4)
		"#,
	)
	.bind(item.id)
	.bind(&item.name)
	.bind(&item.description)
	.bind(item.price)
	.execute(pool)
	.await
	.map(|_| ())
}

async fn insert_soft_delete_item(
	item: &SoftDeleteItem,
	pool: &PgPool,
) -> Result<(), sqlx::Error> {
	sqlx::query(
		r#"
		INSERT INTO soft_delete_item (id, name, description, price, deleted_at)
		VALUES ($1, $2, $3, $4, $5)
		"#,
	)
	.bind(item.id)
	.bind(&item.name)
	.bind(&item.description)
	.bind(item.price)
	.bind(item.deleted_at)
	.execute(pool)
	.await
	.map(|_| ())
}

#[tokio::test]
async fn hard_delete_removes_record() {
	let pool = get_connection_pool().await;
	let item = HardDeleteItem::default();

	insert_hard_delete_item(&item, &pool).await.unwrap();

	// Verify item exists
	let before: Option<HardDeleteItem> = QueryBuilder::<HardDeleteItem>::read()
		.r#where(Expression::Leaf(HardDeleteItemQuery::IdEq(item.id)))
		.build()
		.fetch_optional(&pool)
		.await
		.unwrap();
	assert!(before.is_some());

	// Hard delete the item
	let deleted = QueryBuilder::<HardDeleteItem>::delete()
		.r#where(Expression::Leaf(HardDeleteItemQuery::IdEq(item.id)))
		.build()
		.execute(&pool)
		.await
		.unwrap();

	assert_eq!(deleted, 1);

	// Verify item is gone
	let after: Option<HardDeleteItem> = QueryBuilder::<HardDeleteItem>::read()
		.r#where(Expression::Leaf(HardDeleteItemQuery::IdEq(item.id)))
		.build()
		.fetch_optional(&pool)
		.await
		.unwrap();
	assert!(after.is_none());
}

#[tokio::test]
async fn hard_delete_with_returning() {
	let pool = get_connection_pool().await;
	let item = HardDeleteItem::default();

	insert_hard_delete_item(&item, &pool).await.unwrap();

	// Delete and get the deleted item back
	let deleted_item: HardDeleteItem = QueryBuilder::<HardDeleteItem>::delete()
		.r#where(Expression::Leaf(HardDeleteItemQuery::IdEq(item.id)))
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	assert_eq!(deleted_item, item);

	// Verify it's actually deleted
	let after: Option<HardDeleteItem> = QueryBuilder::<HardDeleteItem>::read()
		.r#where(Expression::Leaf(HardDeleteItemQuery::IdEq(item.id)))
		.build()
		.fetch_optional(&pool)
		.await
		.unwrap();
	assert!(after.is_none());
}

#[tokio::test]
async fn hard_delete_multiple_items() {
	let pool = get_connection_pool().await;
	let item1 = HardDeleteItem {
		name: "cheap".into(),
		price: 10.0,
		..Default::default()
	};
	let item2 = HardDeleteItem {
		name: "cheap".into(),
		price: 15.0,
		..Default::default()
	};
	let item3 = HardDeleteItem {
		name: "expensive".into(),
		price: 100.0,
		..Default::default()
	};

	insert_hard_delete_item(&item1, &pool).await.unwrap();
	insert_hard_delete_item(&item2, &pool).await.unwrap();
	insert_hard_delete_item(&item3, &pool).await.unwrap();

	// Delete cheap items
	let deleted = QueryBuilder::<HardDeleteItem>::delete()
		.r#where(Expression::Leaf(HardDeleteItemQuery::PriceLt(20.0)))
		.build()
		.execute(&pool)
		.await
		.unwrap();

	assert_eq!(deleted, 2);

	// Verify expensive item still exists
	let remaining: Vec<HardDeleteItem> = QueryBuilder::<HardDeleteItem>::read()
		.build()
		.fetch_all(&pool)
		.await
		.unwrap();

	assert_eq!(remaining.len(), 1);
	assert_eq!(remaining[0].id, item3.id);
}

use crate::helpers::{
	UpdateItem,
	UpdateItemQuery,
	UpdateItemUpdate,
};

async fn insert_update_item(
	item: &UpdateItem,
	pool: &PgPool,
) -> Result<(), sqlx::Error> {
	sqlx::query(
		r#"
		INSERT INTO update_item (id, name, description, price, ignored_field, updated_at)
		VALUES ($1, $2, $3, $4, $5, $6)
		"#,
	)
	.bind(item.id)
	.bind(&item.name)
	.bind(&item.description)
	.bind(item.price)
    .bind(&item.ignored_field)
	.bind(item.updated_at)
	.execute(pool)
	.await
	.map(|_| ())
}

#[tokio::test]
async fn update_item_single_field() {
	let pool = get_connection_pool().await;
	let item = UpdateItem::default();

	insert_update_item(&item, &pool).await.unwrap();

	let update = UpdateItemUpdate {
		name: Some("updated name".into()),
		..Default::default()
	};

	let updated: UpdateItem = QueryBuilder::<UpdateItem>::update()
		.model(update)
		.r#where(Expression::Leaf(UpdateItemQuery::IdEq(item.id)))
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	assert_eq!(updated.name, "updated name");
	assert_eq!(updated.description, item.description);
	assert_eq!(updated.price, item.price);
	assert!(updated.updated_at.is_some());
}

#[tokio::test]
async fn update_item_all_fields() {
	let pool = get_connection_pool().await;
	let item = UpdateItem::default();

	insert_update_item(&item, &pool).await.unwrap();

	let update = UpdateItemUpdate {
		name:        Some("completely new".into()),
		description: Some("brand new description".into()),
		price:       Some(999.99),
	};

	let updated: UpdateItem = QueryBuilder::<UpdateItem>::update()
		.model(update)
		.r#where(Expression::Leaf(UpdateItemQuery::IdEq(item.id)))
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	assert_eq!(updated.name, "completely new");
	assert_eq!(updated.description, "brand new description");
	assert_eq!(updated.price, 999.99);
	assert!(updated.updated_at.is_some());
}

#[tokio::test]
async fn update_item_marker_timestamp_set() {
	let pool = get_connection_pool().await;
	let item = UpdateItem::default();

	insert_update_item(&item, &pool).await.unwrap();

	// Wait a bit to ensure timestamp difference
	tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

	let update = UpdateItemUpdate {
		name: Some("trigger marker".into()),
		..Default::default()
	};

	let updated: UpdateItem = QueryBuilder::<UpdateItem>::update()
		.model(update)
		.r#where(Expression::Leaf(UpdateItemQuery::IdEq(item.id)))
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	assert!(updated.updated_at.is_some());
	let updated_time = updated.updated_at.unwrap();

	// Should be recent
	let now = chrono::Utc::now();
	let diff = (now - updated_time).num_seconds().abs();
	assert!(diff < 5); // Within 5 seconds
}

#[tokio::test]
async fn update_item_with_where_price() {
	let pool = get_connection_pool().await;
	let item1 = UpdateItem {
		name: "cheap".into(),
		price: 10.0,
		..Default::default()
	};
	let item2 = UpdateItem {
		name: "expensive".into(),
		price: 200.0,
		..Default::default()
	};

	insert_update_item(&item1, &pool).await.unwrap();
	insert_update_item(&item2, &pool).await.unwrap();

	// Update only cheap items
	let update = UpdateItemUpdate {
		description: Some("budget option".into()),
		..Default::default()
	};

	let count = QueryBuilder::<UpdateItem>::update()
		.model(update)
		.r#where(Expression::Leaf(UpdateItemQuery::PriceLt(50.0)))
		.build()
		.execute(&pool)
		.await
		.unwrap();

	assert_eq!(count, 1);

	// Verify only item1 was updated
	let updated1: UpdateItem = QueryBuilder::<UpdateItem>::read()
		.r#where(Expression::Leaf(UpdateItemQuery::IdEq(item1.id)))
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	assert_eq!(updated1.description, "budget option");

	let unchanged2: UpdateItem = QueryBuilder::<UpdateItem>::read()
		.r#where(Expression::Leaf(UpdateItemQuery::IdEq(item2.id)))
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	assert_ne!(unchanged2.description, "budget option");
}

#[tokio::test]
async fn update_item_multiple_with_returning() {
	let pool = get_connection_pool().await;
	let item1 = UpdateItem {
		description: "old".into(),
		..Default::default()
	};
	let item2 = UpdateItem {
		description: "old".into(),
		..Default::default()
	};

	insert_update_item(&item1, &pool).await.unwrap();
	insert_update_item(&item2, &pool).await.unwrap();

	let update = UpdateItemUpdate {
		description: Some("refreshed".into()),
		..Default::default()
	};

	let updated_items: Vec<UpdateItem> = QueryBuilder::<UpdateItem>::update()
		.model(update)
		.r#where(Expression::Leaf(UpdateItemQuery::DescriptionEq(
			"old".into(),
		)))
		.build()
		.fetch_all(&pool)
		.await
		.unwrap();

	assert_eq!(updated_items.len(), 2);
	assert!(updated_items.iter().all(|i| i.description == "refreshed"));
	assert!(updated_items.iter().all(|i| i.updated_at.is_some()));
}

#[tokio::test]
async fn soft_delete_sets_marker() {
	let pool = get_connection_pool().await;
	let item = SoftDeleteItem::default();

	insert_soft_delete_item(&item, &pool).await.unwrap();

	// Soft delete the item
	let deleted = QueryBuilder::<SoftDeleteItem>::delete()
		.r#where(Expression::Leaf(SoftDeleteItemQuery::IdEq(item.id)))
		.build()
		.execute(&pool)
		.await
		.unwrap();

	assert_eq!(deleted, 1);

	// Item should not appear in normal queries
	let after: Option<SoftDeleteItem> = QueryBuilder::<SoftDeleteItem>::read()
		.r#where(Expression::Leaf(SoftDeleteItemQuery::IdEq(item.id)))
		.build()
		.fetch_optional(&pool)
		.await
		.unwrap();
	assert!(after.is_none());

	// But should appear when including deleted
	let with_deleted: Option<SoftDeleteItem> =
		QueryBuilder::<SoftDeleteItem>::read()
			.include_deleted()
			.r#where(Expression::Leaf(SoftDeleteItemQuery::IdEq(item.id)))
			.build()
			.fetch_optional(&pool)
			.await
			.unwrap();

	assert!(with_deleted.is_some());
	let retrieved = with_deleted.unwrap();
	assert!(retrieved.deleted_at.is_some());
	assert_eq!(retrieved.id, item.id);
}

#[tokio::test]
async fn soft_delete_with_returning() {
	let pool = get_connection_pool().await;
	let item = SoftDeleteItem::default();

	insert_soft_delete_item(&item, &pool).await.unwrap();

	// Delete and get back with deleted_at set
	let deleted_item: SoftDeleteItem = QueryBuilder::<SoftDeleteItem>::delete()
		.r#where(Expression::Leaf(SoftDeleteItemQuery::IdEq(item.id)))
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	assert_eq!(deleted_item.id, item.id);
	assert!(deleted_item.deleted_at.is_some());
}

#[tokio::test]
async fn soft_delete_filters_by_default() {
	let pool = get_connection_pool().await;
	let active_item = SoftDeleteItem::default();
	let deleted_item = SoftDeleteItem {
		deleted_at: Some(chrono::Utc::now()),
		..Default::default()
	};

	insert_soft_delete_item(&active_item, &pool).await.unwrap();
	insert_soft_delete_item(&deleted_item, &pool).await.unwrap();

	// Default query should only return active
	let active_items: Vec<SoftDeleteItem> =
		QueryBuilder::<SoftDeleteItem>::read()
			.build()
			.fetch_all(&pool)
			.await
			.unwrap();

	assert_eq!(active_items.len(), 1);
	assert_eq!(active_items[0].id, active_item.id);

	// With include_deleted, should return both
	let all_items: Vec<SoftDeleteItem> = QueryBuilder::<SoftDeleteItem>::read()
		.include_deleted()
		.build()
		.fetch_all(&pool)
		.await
		.unwrap();

	assert_eq!(all_items.len(), 2);
}

#[tokio::test]
async fn soft_delete_with_where_clause() {
	let pool = get_connection_pool().await;
	let item1 = SoftDeleteItem {
		name: "delete me".into(),
		..Default::default()
	};
	let item2 = SoftDeleteItem {
		name: "keep me".into(),
		..Default::default()
	};

	insert_soft_delete_item(&item1, &pool).await.unwrap();
	insert_soft_delete_item(&item2, &pool).await.unwrap();

	// Soft delete only items matching criteria
	let deleted = QueryBuilder::<SoftDeleteItem>::delete()
		.r#where(Expression::Leaf(SoftDeleteItemQuery::NameEq(
			"delete me".into(),
		)))
		.build()
		.execute(&pool)
		.await
		.unwrap();

	assert_eq!(deleted, 1);

	// Verify only one item remains active
	let active: Vec<SoftDeleteItem> = QueryBuilder::<SoftDeleteItem>::read()
		.build()
		.fetch_all(&pool)
		.await
		.unwrap();

	assert_eq!(active.len(), 1);
	assert_eq!(active[0].id, item2.id);
}

#[tokio::test]
async fn soft_delete_exists_respects_filter() {
	let pool = get_connection_pool().await;
	let deleted_item = SoftDeleteItem {
		deleted_at: Some(chrono::Utc::now()),
		..Default::default()
	};

	insert_soft_delete_item(&deleted_item, &pool).await.unwrap();

	// Should not exist in normal query
	let exists = QueryBuilder::<SoftDeleteItem>::read()
		.r#where(Expression::Leaf(SoftDeleteItemQuery::IdEq(deleted_item.id)))
		.build()
		.exists(&pool)
		.await
		.unwrap();

	assert!(!exists);

	// Should exist when including deleted
	let exists_with_deleted = QueryBuilder::<SoftDeleteItem>::read()
		.include_deleted()
		.r#where(Expression::Leaf(SoftDeleteItemQuery::IdEq(deleted_item.id)))
		.build()
		.exists(&pool)
		.await
		.unwrap();

	assert!(exists_with_deleted);
}

#[tokio::test]
async fn soft_delete_fetch_page_excludes_deleted() {
	let pool = get_connection_pool().await;
	let active1 = SoftDeleteItem::default();
	let active2 = SoftDeleteItem::default();
	let deleted = SoftDeleteItem {
		deleted_at: Some(chrono::Utc::now()),
		..Default::default()
	};

	insert_soft_delete_item(&active1, &pool).await.unwrap();
	insert_soft_delete_item(&active2, &pool).await.unwrap();
	insert_soft_delete_item(&deleted, &pool).await.unwrap();

	let page: Page<SoftDeleteItem> = QueryBuilder::<SoftDeleteItem>::read()
		.paginate(Pagination {
			page:      0,
			page_size: 10,
		})
		.build()
		.fetch_page(&pool)
		.await
		.unwrap();

	assert_eq!(page.total, 2);
	assert_eq!(page.items.len(), 2);
}

use crate::helpers::{
	CreateItem,
	CreateItemCreate,
	CreateItemQuery,
};

#[tokio::test]
async fn insert_item_basic() {
	let pool = get_connection_pool().await;

	let create = CreateItemCreate {
		id:          Uuid::new_v4(),
		name:        "new item".into(),
		description: "a fresh item".into(),
		price:       49.99,
	};

	let inserted: CreateItem = QueryBuilder::<CreateItem>::insert()
		.model(create.clone())
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	assert_eq!(inserted.id, create.id);
	assert_eq!(inserted.name, create.name);
	assert_eq!(inserted.description, create.description);
	assert_eq!(inserted.price, create.price);

	// Verify created_at was set
	let now = chrono::Utc::now();
	let diff = (now - inserted.created_at).num_seconds().abs();
	assert!(diff < 5); // Within 5 seconds
}

#[tokio::test]
async fn insert_item_with_execute() {
	let pool = get_connection_pool().await;

	let create = CreateItemCreate {
		id:          Uuid::new_v4(),
		name:        "execute test".into(),
		description: "testing execute".into(),
		price:       29.99,
	};

	let rows_affected = QueryBuilder::<CreateItem>::insert()
		.model(create.clone())
		.build()
		.execute(&pool)
		.await
		.unwrap();

	assert_eq!(rows_affected, 1);

	// Verify item was actually inserted
	let retrieved: CreateItem = QueryBuilder::<CreateItem>::read()
		.r#where(Expression::Leaf(CreateItemQuery::IdEq(create.id)))
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	assert_eq!(retrieved.id, create.id);
	assert_eq!(retrieved.name, create.name);
}

#[tokio::test]
async fn insert_item_marker_timestamp_auto_set() {
	let pool = get_connection_pool().await;

	let create = CreateItemCreate {
		id:          Uuid::new_v4(),
		name:        "marker test".into(),
		description: "testing created_at".into(),
		price:       99.99,
	};

	let inserted: CreateItem = QueryBuilder::<CreateItem>::insert()
		.model(create)
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	// created_at should be set automatically
	let now = chrono::Utc::now();
	let diff = (now - inserted.created_at).num_seconds().abs();
	assert!(diff < 5); // Within 5 seconds
}

#[tokio::test]
async fn insert_multiple_items() {
	let pool = get_connection_pool().await;

	let create1 = CreateItemCreate {
		id:          Uuid::new_v4(),
		name:        "item 1".into(),
		description: "first".into(),
		price:       10.0,
	};

	let create2 = CreateItemCreate {
		id:          Uuid::new_v4(),
		name:        "item 2".into(),
		description: "second".into(),
		price:       20.0,
	};

	let create3 = CreateItemCreate {
		id:          Uuid::new_v4(),
		name:        "item 3".into(),
		description: "third".into(),
		price:       30.0,
	};

	// Insert all three
	QueryBuilder::<CreateItem>::insert()
		.model(create1.clone())
		.build()
		.execute(&pool)
		.await
		.unwrap();

	QueryBuilder::<CreateItem>::insert()
		.model(create2.clone())
		.build()
		.execute(&pool)
		.await
		.unwrap();

	QueryBuilder::<CreateItem>::insert()
		.model(create3.clone())
		.build()
		.execute(&pool)
		.await
		.unwrap();

	// Verify all were inserted
	let all_items: Vec<CreateItem> = QueryBuilder::<CreateItem>::read()
		.build()
		.fetch_all(&pool)
		.await
		.unwrap();

	assert_eq!(all_items.len(), 3);

	let ids: Vec<Uuid> = all_items.iter().map(|i| i.id).collect();
	assert!(ids.contains(&create1.id));
	assert!(ids.contains(&create2.id));
	assert!(ids.contains(&create3.id));
}

#[tokio::test]
async fn insert_then_read_and_verify() {
	let pool = get_connection_pool().await;

	let create = CreateItemCreate {
		id:          Uuid::new_v4(),
		name:        "verify item".into(),
		description: "for verification".into(),
		price:       77.77,
	};

	// Insert
	let inserted: CreateItem = QueryBuilder::<CreateItem>::insert()
		.model(create.clone())
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	// Read back
	let retrieved: CreateItem = QueryBuilder::<CreateItem>::read()
		.r#where(Expression::Leaf(CreateItemQuery::IdEq(create.id)))
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	// Should match
	assert_eq!(retrieved.id, inserted.id);
	assert_eq!(retrieved.name, inserted.name);
	assert_eq!(retrieved.description, inserted.description);
	assert_eq!(retrieved.price, inserted.price);

	// Timestamps should be very close (both set by DB)
	let diff = (retrieved.created_at - inserted.created_at)
		.num_milliseconds()
		.abs();
	assert!(diff < 1000); // Within 1 second
}

#[tokio::test]
async fn insert_with_special_characters() {
	let pool = get_connection_pool().await;

	let create = CreateItemCreate {
		id:          Uuid::new_v4(),
		name:        "Special \"Item\" with 'quotes'".into(),
		description: "Has $pecial ch@rs & symbols!".into(),
		price:       12.34,
	};

	let inserted: CreateItem = QueryBuilder::<CreateItem>::insert()
		.model(create.clone())
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	assert_eq!(inserted.name, create.name);
	assert_eq!(inserted.description, create.description);
}

#[tokio::test]
async fn insert_fetch_optional() {
	let pool = get_connection_pool().await;

	let create = CreateItemCreate {
		id:          Uuid::new_v4(),
		name:        "optional test".into(),
		description: "testing fetch_optional".into(),
		price:       55.55,
	};

	let maybe_inserted: Option<CreateItem> =
		QueryBuilder::<CreateItem>::insert()
			.model(create.clone())
			.build()
			.fetch_optional(&pool)
			.await
			.unwrap();

	assert!(maybe_inserted.is_some());
	let inserted = maybe_inserted.unwrap();
	assert_eq!(inserted.id, create.id);
	assert_eq!(inserted.name, create.name);
}

#[tokio::test]
async fn insert_creates_new_record() {
	let pool = get_connection_pool().await;
	let id = Uuid::new_v4();

	let create = CreateItemCreate {
		id,
		name: "new item".into(),
		description: "test description".into(),
		price: 49.99,
	};

	let rows_affected = QueryBuilder::<CreateItem>::insert()
		.model(create)
		.build()
		.execute(&pool)
		.await
		.unwrap();

	assert_eq!(rows_affected, 1);

	// Verify the item was created
	let inserted: CreateItem = sqlx::query_as(
		"SELECT id, name, description, price, created_at FROM create_item \
		 WHERE id = $1",
	)
	.bind(id)
	.fetch_one(&pool)
	.await
	.unwrap();

	assert_eq!(inserted.id, id);
	assert_eq!(inserted.name, "new item");
	assert_eq!(inserted.description, "test description");
	assert_eq!(inserted.price, 49.99);
}

#[tokio::test]
async fn insert_with_returning_fetches_created_record() {
	let pool = get_connection_pool().await;
	let id = Uuid::new_v4();

	let create = CreateItemCreate {
		id,
		name: "returnable item".into(),
		description: "should return".into(),
		price: 99.99,
	};

	let created: CreateItem = QueryBuilder::<CreateItem>::insert()
		.model(create)
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	assert_eq!(created.id, id);
	assert_eq!(created.name, "returnable item");
	assert_eq!(created.description, "should return");
	assert_eq!(created.price, 99.99);

	// Verify created_at is set and recent
	let now = chrono::Utc::now();
	let diff = (now - created.created_at).num_seconds().abs();
	assert!(diff < 5); // Within 5 seconds
}

#[tokio::test]
async fn insert_marker_automatically_set() {
	let pool = get_connection_pool().await;
	let id = Uuid::new_v4();

	let create = CreateItemCreate {
		id,
		name: "marker test".into(),
		description: "test marker".into(),
		price: 25.50,
	};

	let created: CreateItem = QueryBuilder::<CreateItem>::insert()
		.model(create)
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	// Verify created_at was automatically set to NOW()
	let now = chrono::Utc::now();
	let diff = (now - created.created_at).num_seconds().abs();
	assert!(diff < 5); // Should be very recent (within 5 seconds)
}

#[tokio::test]
async fn insert_multiple_items_with_fetch_all() {
	let pool = get_connection_pool().await;
	let id1 = Uuid::new_v4();
	let id2 = Uuid::new_v4();

	let create1 = CreateItemCreate {
		id:          id1,
		name:        "item 1".into(),
		description: "first".into(),
		price:       10.0,
	};

	let create2 = CreateItemCreate {
		id:          id2,
		name:        "item 2".into(),
		description: "second".into(),
		price:       20.0,
	};

	// Insert both items
	QueryBuilder::<CreateItem>::insert()
		.model(create1)
		.build()
		.execute(&pool)
		.await
		.unwrap();

	QueryBuilder::<CreateItem>::insert()
		.model(create2)
		.build()
		.execute(&pool)
		.await
		.unwrap();

	// Fetch all items
	let all_items: Vec<CreateItem> = sqlx::query_as(
		"SELECT id, name, description, price, created_at FROM create_item \
		 ORDER BY price",
	)
	.fetch_all(&pool)
	.await
	.unwrap();

	assert_eq!(all_items.len(), 2);
	assert_eq!(all_items[0].name, "item 1");
	assert_eq!(all_items[1].name, "item 2");
}

#[tokio::test]
async fn insert_with_fetch_optional_returns_some() {
	let pool = get_connection_pool().await;
	let id = Uuid::new_v4();

	let create = CreateItemCreate {
		id,
		name: "optional item".into(),
		description: "maybe".into(),
		price: 15.75,
	};

	let maybe_created: Option<CreateItem> =
		QueryBuilder::<CreateItem>::insert()
			.model(create)
			.build()
			.fetch_optional(&pool)
			.await
			.unwrap();

	assert!(maybe_created.is_some());
	let created = maybe_created.unwrap();
	assert_eq!(created.id, id);
	assert_eq!(created.name, "optional item");
}

#[tokio::test]
async fn insert_preserves_all_field_values() {
	let pool = get_connection_pool().await;
	let id = Uuid::new_v4();

	let create = CreateItemCreate {
		id,
		name: "complete item".into(),
		description: "full description text".into(),
		price: 123.45,
	};

	let created: CreateItem = QueryBuilder::<CreateItem>::insert()
		.model(create.clone())
		.build()
		.fetch_one(&pool)
		.await
		.unwrap();

	// Verify all fields match
	assert_eq!(created.id, create.id);
	assert_eq!(created.name, create.name);
	assert_eq!(created.description, create.description);
	assert_eq!(created.price, create.price);
}
