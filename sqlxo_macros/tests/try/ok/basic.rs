use sqlxo_macros::Query;
use sqlxo_traits::QueryContext;
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Query)]
#[sqlxo(table_name = "item")]
pub struct Item {
    pub id: Uuid,
    pub name: String,
    pub active: bool,
    pub price: f32,
}

fn main() {
    let _ = <Item as QueryContext>::TABLE;

    enum _Use {
        A(ItemQuery),
        B(ItemSort),
    }
}
