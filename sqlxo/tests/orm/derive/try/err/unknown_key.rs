#![feature(inherent_associated_types)]
#![allow(incomplete_features)]

use sqlx::FromRow;
use sqlxo_macros::Query;

#[derive(Debug, Clone, FromRow, Query)]
#[sqlxo(foo = "bar")]
pub struct T {
	pub name: String,
}
fn main() {}
