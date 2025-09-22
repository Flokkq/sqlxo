use filter_macros::Query;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Query)]
pub struct T(String, i32);

fn main() {}
