use filter_macros::Query;
use sqlx::FromRow;

#[derive(Debug, FromRow, Query)]
pub enum E {
    A,
}
fn main() {}
