use chrono::Utc;
use filter_macros::Query;
use uuid::Uuid;

#[derive(Query)]
pub struct X {
    pub id: Option<Uuid>,
    pub name: Option<String>,
    pub count: Option<i32>,
    pub at: Option<chrono::DateTime<Utc>>,
    pub flag: Option<bool>,
}

fn main() {}
