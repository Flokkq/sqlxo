use sqlxo_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
#[sqlxo = "oops"]
pub struct T {
    pub name: String,
}
fn main() {}
