#![feature(inherent_associated_types)]
#![feature(trait_alias)]
#![feature(specialization)]
#![allow(incomplete_features)]

use chrono::{
	DateTime,
	Utc,
};
use serde::{
	Deserialize,
	Serialize,
};
use sqlxo::{
	bind,
	web::WebFilter,
	JoinValue,
	Query,
	WebQuery,
};
use utoipa::{
	OpenApi,
	ToSchema,
};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow, Query)]
pub struct Supplier {
	#[primary_key]
	pub id:   Uuid,
	pub name: String,
}

#[derive(Debug, Clone, sqlx::FromRow, Query)]
pub struct Material {
	#[primary_key]
	pub id:          Uuid,
	pub name:        String,
	pub description: String,
	#[foreign_key(to = "supplier.id")]
	pub supplier_id: Option<Uuid>,

	#[sqlxo(belongs_to)]
	#[sqlx(skip)]
	pub supplier: JoinValue<Supplier>,
}

#[derive(Debug, Clone, sqlx::FromRow, Query)]
pub struct Item {
	#[primary_key]
	pub id:          Uuid,
	pub name:        String,
	pub description: String,
	pub price:       f32,
	pub amount:      i32,
	pub active:      bool,
	pub due_date:    DateTime<Utc>,

	#[foreign_key(to = "material.id")]
	pub material_id: Option<Uuid>,

	#[sqlxo(belongs_to)]
	#[sqlx(skip)]
	pub material: JoinValue<Material>,
}

#[bind(Supplier)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, WebQuery)]
pub struct SupplierDto {
	pub id:   Uuid,
	pub name: String,
}

#[bind(Material)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, WebQuery)]
pub struct MaterialDto {
	pub id:          Uuid,
	pub name:        String,
	pub description: String,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[sqlxo(webquery_join)]
	pub supplier: Option<SupplierDto>,
}

#[bind(Item)]
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, WebQuery)]
pub struct ItemDto {
	pub id:             Uuid,
	#[schema(min_length = 1, max_length = 64)]
	#[sqlxo(field = "name")]
	pub different_name: String,
	pub description:    String,
	pub price:          f32,
	pub amount:         i32,
	pub active:         bool,
	pub due_date:       DateTime<Utc>,

	#[serde(skip_serializing_if = "Option::is_none")]
	#[sqlxo(webquery_join)]
	pub material: Option<MaterialDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct ItemUpdateModel {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub name:        Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub description: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub price:       Option<f32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub amount:      Option<i32>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub active:      Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ItemUpdatePayload {
	pub filter: WebFilter<ItemDto>,
	pub update: ItemUpdateModel,
}

#[allow(dead_code)]
#[utoipa::path(
    get,
    path = "/items/sqlxo",
    params(WebFilter<ItemDto>),
    responses((status = 200, description = "Filtered items", body = [ItemDto])),
    tag = "items"
)]
fn sqlxo_items(_query: WebFilter<ItemDto>) -> Vec<ItemDto> {
	Vec::new()
}

#[allow(dead_code)]
#[utoipa::path(
    patch,
    path = "/items/sqlxo",
    request_body = ItemUpdatePayload,
    responses((status = 200, description = "Updated rows", body = usize)),
    tag = "items"
)]
fn sqlxo_items_update(_payload: ItemUpdatePayload) -> usize {
	0
}

#[allow(dead_code)]
#[utoipa::path(
    delete,
    path = "/items/sqlxo",
    params(WebFilter<ItemDto>),
    responses((status = 200, description = "Deleted rows", body = u64)),
    tag = "items"
)]
fn sqlxo_items_delete(_query: WebFilter<ItemDto>) -> u64 {
	0
}

#[derive(OpenApi)]
#[openapi(
    info(title = "SQL Filter API", version = "1.0.0"),
    paths(sqlxo_items, sqlxo_items_update, sqlxo_items_delete),
    components(
        schemas(
            ItemDto,
            MaterialDto,
            SupplierDto,
            ItemUpdateModel,
            ItemUpdatePayload,

            ItemDtoLeaf,
            ItemDtoSortField,
            ItemDtoAggregateLeaf,
            ItemDtoJoinPath,

            MaterialDtoLeaf,
            MaterialDtoSortField,
            MaterialDtoJoinPath,

            SupplierDtoJoinPath,

            sqlxo::WebSortDirection,
            sqlxo::web::WebPagination,
            sqlxo::web::WebSearch,
            sqlxo::web::GenericWebExpression<ItemDtoLeaf>,
            sqlxo::web::GenericWebExpression<ItemDtoAggregateLeaf>,
            sqlxo::web::GenericWebSort<ItemDtoSortField>,

            sqlxo::web::WebFilter<ItemDto>
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
