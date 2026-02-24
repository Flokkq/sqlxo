#![feature(inherent_associated_types)]
#![allow(incomplete_features)]

use sqlx::FromRow;
use sqlxo_macros::Query;

#[derive(Debug, Clone, FromRow, Query)]
pub enum E {
	A,
}
fn main() {}
