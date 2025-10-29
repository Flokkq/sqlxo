use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlo_macros::WebQuery;
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
    path = "/items/sqlo",
    request_body = sqlo_traits::DtoFilter<ItemDto>,
    responses((status = 200, description = "Filtered items", body = [ItemDto])),
    tag = "items"
)]
fn sqlo_items(_payload: sqlo_traits::DtoFilter<ItemDto>) -> Vec<ItemDto> {
    Vec::new()
}

#[derive(OpenApi)]
#[openapi(
    info(title = "SQL Filter API", version = "1.0.0"),
    paths(sqlo_items),
    components(
        schemas(
            ItemDto,

            ItemDtoLeaf,
            ItemDtoSortField,

            sqlo_traits::DtoSortDir,
            sqlo_traits::DtoPage,

            sqlo_traits::GenericDtoExpression<ItemDtoLeaf>,
            sqlo_traits::GenericDtoSort<ItemDtoSortField>,

            sqlo_traits::DtoFilter<ItemDto>
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
