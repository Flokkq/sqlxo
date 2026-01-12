use serde::{
	Deserialize,
	Serialize,
};
use sqlx::prelude::FromRow;
use sqlxo::{
	bind,
	Delete,
	Query,
	SoftDelete,
	WebQuery,
};
use sqlxo_macros::{
	Create,
	Update,
};
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

impl Default for Item {
	fn default() -> Self {
		Item {
			id:          Uuid::new_v4(),
			name:        "test".into(),
			description: "item description".into(),
			price:       23.5f32,
			amount:      2,
			active:      true,
			due_date:    chrono::Utc::now(),
			material_id: None,
		}
	}
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

// Hard delete test model
#[allow(dead_code)]
#[derive(Debug, FromRow, Clone, Query, Delete, PartialEq)]
#[sqlxo(table_name = "hard_delete_item")]
pub struct HardDeleteItem {
	#[primary_key]
	pub id:          Uuid,
	pub name:        String,
	pub description: String,
	pub price:       f32,
}

impl Default for HardDeleteItem {
	fn default() -> Self {
		Self {
			id:          Uuid::new_v4(),
			name:        "hard delete test".into(),
			description: "test item".into(),
			price:       50.0,
		}
	}
}

// Soft delete test model
#[allow(dead_code)]
#[derive(Debug, FromRow, Clone, Query, SoftDelete, PartialEq)]
#[sqlxo(table_name = "soft_delete_item")]
pub struct SoftDeleteItem {
	#[primary_key]
	pub id:          Uuid,
	pub name:        String,
	pub description: String,
	pub price:       f32,
	#[sqlxo(delete_marker)]
	pub deleted_at:
		Option<sqlx::types::chrono::DateTime<sqlx::types::chrono::Utc>>,
}

impl Default for SoftDeleteItem {
	fn default() -> Self {
		Self {
			id:          Uuid::new_v4(),
			name:        "soft delete test".into(),
			description: "test item".into(),
			price:       75.0,
			deleted_at:  None,
		}
	}
}
// Update test model
#[allow(dead_code)]
#[derive(Debug, FromRow, Clone, Query, Update, PartialEq)]
#[sqlxo(table_name = "update_item")]
pub struct UpdateItem {
	#[primary_key]
	pub id:          Uuid,
	pub name:        String,
	pub description: String,
	pub price:       f32,

	#[sqlxo(update_ignore)]
	pub ignored_field: String,

	#[sqlxo(update_marker)]
	pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Default for UpdateItem {
	fn default() -> Self {
		Self {
			id:            Uuid::new_v4(),
			name:          "update test".into(),
			description:   "test item".into(),
			ignored_field: "ignored".into(),
			price:         75.0,
			updated_at:    None,
		}
	}
}

#[allow(dead_code)]
#[derive(Debug, FromRow, Clone, Query, Create, PartialEq)]
#[sqlxo(table_name = "create_item")]
pub struct CreateItemCreation {
	#[primary_key(manual)]
	pub id:          Uuid,
	pub name:        String,
	pub description: String,
	pub price:       f32,
	#[sqlxo(insert_marker)]
	pub created_at:  chrono::DateTime<chrono::Utc>,
}

impl Default for CreateItem {
	fn default() -> Self {
		Self {
			id:          Uuid::new_v4(),
			name:        "create test".into(),
			description: "test item".into(),
			price:       85.0,
			created_at:  chrono::Utc::now(),
		}
	}
}
