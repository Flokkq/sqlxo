use sqlo_macros::Query;
use sqlo_traits::QueryContext;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
pub struct SnakeCaseName {
    pub name: String,
}

fn main() {
    assert_eq!(<SnakeCaseName as QueryContext>::TABLE, "snake_case_name");
}
