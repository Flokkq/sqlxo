use sqlo_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
#[sqlo = "oops"]
pub struct T {
    pub name: String,
}
fn main() {}
