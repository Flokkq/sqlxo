#![feature(inherent_associated_types)]
#![allow(incomplete_features)]

use sqlx::FromRow;
use sqlxo_macros::Query;

#[derive(Debug, Clone, FromRow, Query)]
#[sqlxo(table_name = "a", table_name = "b")]
pub struct T {
	pub name: String,
}
fn main() {}
