#![feature(inherent_associated_types)]

use sqlxo_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
#[sqlxo(table_name = "a", table_name = "b")]
pub struct T {
    pub name: String,
}
fn main() {}
