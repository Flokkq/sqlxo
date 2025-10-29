use sqlxo_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
#[sqlxo(table_name = 123)]
pub struct T {
    pub name: String,
}
fn main() {}
