use claims::assert_some_eq;
use sqlx::migrate;
use sqlx::postgres::PgConnectOptions;
use sqlx::postgres::PgPoolOptions;
use sqlx::postgres::PgSslMode;
use sqlx::PgPool;
use sqlxo::and;
use sqlxo::blocks::Expression;
use sqlxo::blocks::Page;
use sqlxo::blocks::Pagination;
use sqlxo::or;
use sqlxo::order_by;
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

	let maybe: Option<Item> = QueryBuilder::<Item>::from_ctx()
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

	let page: Page<Item> = QueryBuilder::<Item>::from_ctx()
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

	let exists: bool = QueryBuilder::<Item>::from_ctx()
		.r#where(Expression::Leaf(ItemQuery::NameEq("test".into())))
		.build()
		.exists(&pool)
		.await
		.unwrap();

	assert!(exists);
}
