use sqlo_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
#[sqlo(table_name = 123)]
pub struct T {
    pub name: String,
}
fn main() {}
