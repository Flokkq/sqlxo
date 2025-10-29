use sqlo_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
#[sqlo(table_name = "a", table_name = "b")]
pub struct T {
    pub name: String,
}
fn main() {}
