use filter_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Query)]
#[filter(table_name = "a")]
pub struct T {
    pub name: String,
}
fn main() {}
