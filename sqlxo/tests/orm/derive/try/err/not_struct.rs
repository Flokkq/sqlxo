#![feature(inherent_associated_types)]

use sqlxo_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
pub enum E {
    A,
}
fn main() {}
