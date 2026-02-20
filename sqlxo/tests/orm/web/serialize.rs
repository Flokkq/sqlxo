use claims::assert_some;
use serde_json::{
	json,
	Value,
};
use sqlxo::{
	blocks::{
		BuildableJoin,
		SelectType,
	},
	web::WebFilter,
	Buildable,
	JoinKind,
	QueryBuilder,
	ReadQueryPlan,
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

	let plan: ReadQueryPlan<Item> =
		QueryBuilder::<Item>::from_dto::<ItemDto>(&f)
			.join(ItemJoin::ItemToMaterialByMaterialId, JoinKind::Left)
			.build();

	assert_eq!(
		plan.sql(SelectType::Star).trim_start().normalize(),
		r#"
        SELECT *, "material__"."id" AS "__sqlxo_material__id",
            "material__"."name" AS "__sqlxo_material__name",
            "material__"."long_name" AS "__sqlxo_material__long_name",
            "material__"."description" AS "__sqlxo_material__description",
            "material__"."supplier_id" AS "__sqlxo_material__supplier_id"
        FROM item
        LEFT JOIN material AS "material__" ON "item"."material_id" = "material__"."id"
        WHERE ("item"."name" LIKE $1 AND ("item"."price" > $2 OR "item"."description" <> $3))
        ORDER BY "item"."name" ASC, "item"."description" DESC
        LIMIT $4 OFFSET $5
    "#
		.normalize()
	);
}
