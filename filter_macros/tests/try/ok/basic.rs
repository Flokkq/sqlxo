use filter_macros::Query;
use uuid::Uuid;

#[derive(Query)]
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
