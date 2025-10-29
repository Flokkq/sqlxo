use chrono::Utc;
use sqlo_macros::Query;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Query)]
pub struct X {
    pub id: Option<Uuid>,
    pub name: Option<String>,
    pub count: Option<i32>,
    pub at: Option<chrono::DateTime<Utc>>,
    pub flag: Option<bool>,
}

fn main() {}
