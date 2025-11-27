use serde::{
	Deserialize,
	Serialize,
};
use sqlx::prelude::FromRow;
use sqlxo::{
	bind,
	WebQuery,
};
use sqlxo_macros::Query;
use uuid::Uuid;

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
	pub id:          Uuid,
	pub name:        String,
	pub description: String,
	pub price:       f32,
	pub amount:      i32,
	pub active:      bool,
	pub due_date:    sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,

	#[foreign_key(to = "material.id")]
	pub material_id: Option<Uuid>,
}

#[allow(dead_code)]
#[bind(Item)]
#[derive(Debug, Clone, WebQuery, Deserialize, Serialize)]
pub struct ItemDto {
	pub id:             Uuid,
	#[sqlxo(field = "name")]
	pub different_name: String,
	pub description:    String,
	pub price:          f32,
	pub amount:         i32,
	pub active:         bool,
	pub due_date:       sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>,
}

#[allow(dead_code)]
#[derive(Debug, FromRow, Clone, Query)]
pub struct Material {
	#[primary_key]
	pub id: Uuid,

	pub name:        String,
	pub long_name:   String,
	pub description: String,
}
