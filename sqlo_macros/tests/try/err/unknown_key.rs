use sqlo_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
#[sqlo(foo = "bar")]
pub struct T {
    pub name: String,
}
fn main() {}
