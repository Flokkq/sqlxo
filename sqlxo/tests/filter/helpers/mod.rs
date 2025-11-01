use sqlx::prelude::FromRow;
use sqlxo_macros::Query;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, FromRow, Clone, Query, PartialEq)]
pub struct Item {
	#[primary_key]
	pub id:          Uuid,
	pub name:        String,
	pub description: String,
	pub price:       f32,
	pub amount:      i32,
	pub active:      bool,
	pub due_date:    chrono::DateTime<chrono::Utc>,

	#[foreign_key(to = "material.id")]
	pub material_id: Option<Uuid>,
}
