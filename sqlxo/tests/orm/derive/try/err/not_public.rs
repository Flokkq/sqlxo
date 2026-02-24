#![feature(inherent_associated_types)]
#![allow(incomplete_features)]

use sqlx::FromRow;
use sqlxo_macros::Query;

#[derive(Debug, Clone, FromRow, Query)]
struct Private {
	pub name: String,
}
fn main() {}
