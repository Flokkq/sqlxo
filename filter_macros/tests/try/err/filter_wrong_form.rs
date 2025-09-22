use filter_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
#[filter = "oops"]
pub struct T {
    pub name: String,
}
fn main() {}
