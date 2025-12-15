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
use sqlxo::FetchablePlan;
use sqlxo::QueryBuilder;
use uuid::Uuid;
use crate::helpers::{HardDeleteItem, HardDeleteItemQuery, SoftDeleteItem, SoftDeleteItemQuery};
use sqlxo::ExecutablePlan;

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


async fn insert_hard_delete_item(item: &HardDeleteItem, pool: &PgPool) -> Result<(), sqlx::Error> {
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

async fn insert_soft_delete_item(item: &SoftDeleteItem, pool: &PgPool) -> Result<(), sqlx::Error> {
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
	let with_deleted: Option<SoftDeleteItem> = QueryBuilder::<SoftDeleteItem>::read()
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
	let active_items: Vec<SoftDeleteItem> = QueryBuilder::<SoftDeleteItem>::read()
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
		.r#where(Expression::Leaf(SoftDeleteItemQuery::NameEq("delete me".into())))
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