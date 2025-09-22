use filter_macros::Query;
use sqlx::FromRow;

#[derive(Debug, FromRow, Query)]
#[filter(table_name = "a", table_name = "b")]
pub struct T {
    pub name: String,
}
fn main() {}
