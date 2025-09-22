use filter_macros::Query;
use filter_traits::QueryContext;
use sqlx::FromRow;

#[derive(Debug, FromRow, Query)]
pub struct SnakeCaseName {
    pub name: String,
}

fn main() {
    assert_eq!(<SnakeCaseName as QueryContext>::TABLE, "snake_case_name");
}
