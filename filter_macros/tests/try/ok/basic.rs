use filter_macros::Query;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, FromRow, Query)]
#[filter(table_name = "item")]
pub struct Item {
    pub id: Uuid,
    pub name: String,
    pub active: bool,
    pub price: f32,
}

fn main() {
    let _ = ITEM_TABLE;

    enum _Use {
        A(ItemQuery),
        B(ItemSort),
    }
}
