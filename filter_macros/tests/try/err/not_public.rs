use filter_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
struct Private {
    pub name: String,
}
fn main() {}
