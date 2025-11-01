use chrono::{
	DateTime,
	Utc,
};
use serde::{
	Deserialize,
	Serialize,
};
use sqlxo_macros::WebQuery;
use utoipa::{
	OpenApi,
	ToSchema,
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, WebQuery)]
pub struct ItemDto {
	pub id:          Uuid,
	pub name:        String,
	pub description: String,
	pub price:       f32,
	pub amount:      i32,
	pub active:      bool,
	pub due_date:    DateTime<Utc>,
}

#[allow(dead_code)]
#[utoipa::path(
    post,
    path = "/items/sqlxo",
    request_body = sqlxo_traits::DtoFilter<ItemDto>,
    responses((status = 200, description = "Filtered items", body = [ItemDto])),
    tag = "items"
)]
fn sqlxo_items(_payload: sqlxo_traits::DtoFilter<ItemDto>) -> Vec<ItemDto> {
	Vec::new()
}

#[derive(OpenApi)]
#[openapi(
    info(title = "SQL Filter API", version = "1.0.0"),
    paths(sqlxo_items),
    components(
        schemas(
            ItemDto,

            ItemDtoLeaf,
            ItemDtoSortField,

            sqlxo_traits::DtoSortDir,
            sqlxo_traits::DtoPage,

            sqlxo_traits::GenericDtoExpression<ItemDtoLeaf>,
            sqlxo_traits::GenericDtoSort<ItemDtoSortField>,

            sqlxo_traits::DtoFilter<ItemDto>
        )
    ),
    tags((name = "items", description = "Filtering Items with WebQuery payload"))
)]
struct ApiDoc;

fn main() {
	let doc = ApiDoc::openapi();
	let json = serde_json::to_string_pretty(&doc).unwrap();
	println!("{json}");
}
