#![feature(inherent_associated_types)]
#![allow(incomplete_features)]

use sqlxo_macros::Query;

#[derive(Debug, Clone, Query)]
#[sqlxo(table_name = "a")]
pub struct T {
	pub name: String,
}
fn main() {}
