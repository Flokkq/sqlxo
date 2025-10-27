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

#[allow(dead_code)]
#[utoipa::path(
    post,
    path = "/items/filter",
    request_body = filter_traits::DtoFilter<ItemDto>,
    responses((status = 200, description = "Filtered items", body = [ItemDto])),
    tag = "items"
)]
fn filter_items(_payload: filter_traits::DtoFilter<ItemDto>) -> Vec<ItemDto> {
    Vec::new()
}

#[derive(OpenApi)]
#[openapi(
    info(title = "SQL Filter API", version = "1.0.0"),
    paths(filter_items),
    components(
        schemas(
            ItemDto,

            ItemDtoLeaf,
            ItemDtoSortField,

            filter_traits::DtoSortDir,
            filter_traits::DtoPage,

            filter_traits::GenericDtoExpression<ItemDtoLeaf>,
            filter_traits::GenericDtoSort<ItemDtoSortField>,

            filter_traits::DtoFilter<ItemDto>
        )
    ),
    tags((name = "items", description = "Filtering Items with WebQuery payload"))
)]
struct ApiDoc;

fn main() {
    let doc = ApiDoc::openapi();
    let json = serde_json::to_string_pretty(&doc).unwrap();
    println!("{}", json);
}
