#![feature(inherent_associated_types)]

use chrono::Utc;

use sqlx::FromRow;
use sqlxo_macros::Query;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Query)]
pub struct X {
	pub id:    Option<Uuid>,
	pub name:  Option<String>,
	pub count: Option<i32>,
	pub at:    Option<chrono::DateTime<Utc>>,
	pub flag:  Option<bool>,
}

fn main() {}
