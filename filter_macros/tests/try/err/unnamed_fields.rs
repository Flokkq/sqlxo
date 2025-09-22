use filter_macros::Query;
use sqlx::FromRow;

#[derive(Debug, FromRow, Query)]
pub struct T(String, i32);

fn main() {}
