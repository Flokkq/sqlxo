#![feature(inherent_associated_types)]

use sqlx::FromRow;
use sqlxo_macros::Query;
use sqlxo_traits::QueryContext;

#[derive(Debug, Clone, FromRow, Query)]
pub struct SnakeCaseName {
	pub name: String,
}

fn main() {
	assert_eq!(<SnakeCaseName as QueryContext>::TABLE, "snake_case_name");
}
