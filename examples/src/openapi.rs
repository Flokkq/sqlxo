use chrono::{DateTime, Utc};
use filter_macros::WebQuery;
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, WebQuery)]
pub struct ItemDto {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub price: f32,
    pub amount: i32,
    pub active: bool,
    pub due_date: DateTime<Utc>,
}

pub use ItemDto as _KeepRustHappy;

#[allow(dead_code)]
#[utoipa::path(
    post,
    path = "/items/filter",
    request_body = ItemDtoFilter,
    responses(
        (status = 200, description = "Filtered items", body = [ItemDto])),
    tag = "items"
)]
fn filter_items(_payload: ItemDtoFilter) -> Vec<ItemDto> {
    Vec::new()
}

#[derive(OpenApi)]
#[openapi(
    info(title = "SQL Filter API", version = "1.0.0"),
    paths(filter_items),
    components(
        schemas(
            ItemDto,
            ItemDtoFilter,
            ItemDtoExpr,
            ItemDtoLeaf,
            ItemDtoSortWeb,
            ItemDtoSortDir,
            ItemDtoPage
        )
    ),
    tags(
        (name = "items", description = "Filtering Items with WebQuery payload")
    )
)]
struct ApiDoc;

fn main() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&doc).expect("serialize openapi");
    println!("{}", json);
}
