use filter_macros::Query;
use sqlx::FromRow;

#[derive(Debug, FromRow, Query)]
#[filter(table_name = 123)]
pub struct T {
    pub name: String,
}
fn main() {}
