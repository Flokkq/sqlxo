use claims::assert_some;
use serde_json::{
	json,
	Value,
};
use sqlxo::{
	blocks::BuildType,
	web::WebFilter,
	JoinKind,
	QueryBuilder,
	QueryPlan,
};

use crate::helpers::{
	Item,
	ItemDto,
	ItemJoin,
	NormalizeString,
};

#[test]
fn deserialize_itemdto_sqlxo_json() {
	let json: Value = json!({
		"filter": {
			"and": [
				{ "different_name": { "like": "%Sternlampe%" } },
				{ "or": [
					{ "price": { "gt": 18.00 } },
					{ "description": { "neq": "von Hohlweg" } }
				]}
			]
		},
		"sort": [
			{ "different_name": "asc" },
			{ "description": "desc" }
		],
		"page": { "pageSize": 10, "pageNo": 1 }
	});

	let f: WebFilter<ItemDto> =
		serde_json::from_value(json).expect("valid ItemDtoFilter");

	assert_some!(f.page);
	assert_eq!(f.page.unwrap().page_size, 10);
	assert_eq!(f.page.unwrap().page, 1);
}

#[test]
fn query_builder_from_dto_filter() {
	let json: Value = json!({
		"filter": {
			"and": [
				{ "different_name": { "like": "%Sternlampe%" } },
				{ "or": [
					{ "price": { "gt": 18.00 } },
					{ "description": { "neq": "von Hohlweg" } }
				]}
			]
		},
		"sort": [
			{ "different_name": "asc" },
			{ "description": "desc" }
		],
		"page": { "pageSize": 10, "pageNo": 1 }
	});

	let f: WebFilter<ItemDto> =
		serde_json::from_value(json).expect("valid ItemDtoFilter");

	let plan: QueryPlan<Item> = QueryBuilder::<Item>::from_dto::<ItemDto>(&f)
		.join(ItemJoin::ItemToMaterialByMaterialId(JoinKind::Left))
		.build();

	assert_eq!(
		plan.sql(BuildType::Raw).trim_start().normalize(),
		r#"
        LEFT JOIN material ON "item"."material_id" = "material"."id"
        WHERE (name LIKE $1 AND (price > $2 OR description <> $3))
        ORDER BY name ASC, description DESC
        LIMIT $4 OFFSET $5
    "#
		.normalize()
	);
}
