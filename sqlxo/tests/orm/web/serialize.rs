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
				{ "differentName": { "like": "%Sternlampe%" } },
				{ "or": [
					{ "price": { "gt": 18.00 } },
					{ "description": { "neq": "von Hohlweg" } }
				]}
			]
		},
		"sort": [
			{ "differentName": "asc" },
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
				{ "differentName": { "like": "%Sternlampe%" } },
				{ "or": [
					{ "price": { "gt": 18.00 } },
					{ "description": { "neq": "von Hohlweg" } }
				]}
			]
		},
		"sort": [
			{ "differentName": "asc" },
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
        SELECT "item".*, "material__"."id" AS "__sqlxo_material__id",
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

#[test]
fn web_payload_applies_joins_search_and_having() {
	let json: Value = json!({
		"joins": [
			{ "path": ["material"], "kind": "left" }
		],
		"filter": {
			"differentName": { "like": "%Sternlampe%" }
		},
		"search": { "query": "bolt", "includeRank": false },
		"having": {
			"and": [
				{ "count": { "gt": 5 } },
				{ "priceSum": { "gt": 25.0 } }
			]
		}
	});

	let f: WebFilter<ItemDto> =
		serde_json::from_value(json).expect("valid ItemDtoFilter");

	let plan: ReadQueryPlan<Item> =
		QueryBuilder::<Item>::from_dto::<ItemDto>(&f).build();
	let sql = plan.sql(SelectType::Star).trim_start().normalize();

	assert_eq!(
		sql,
		r#"
        SELECT "item".*, "material__"."id" AS "__sqlxo_material__id",
            "material__"."name" AS "__sqlxo_material__name",
            "material__"."long_name" AS "__sqlxo_material__long_name",
            "material__"."description" AS "__sqlxo_material__description",
            "material__"."supplier_id" AS "__sqlxo_material__supplier_id"
        FROM item
        LEFT JOIN material AS "material__" ON "item"."material_id" = "material__"."id"
        WHERE "item"."name" LIKE $1 AND ((setweight(to_tsvector('english', "item"."name"), 'A') || setweight(to_tsvector('english', "item"."description"), 'B')) @@ (websearch_to_tsquery('english', $2))) AND "item"."id" IN (SELECT "item"."id"
            FROM item
            LEFT JOIN material AS "material__" ON "item"."material_id" = "material__"."id"
            WHERE ("item"."name" LIKE $3) AND ((setweight(to_tsvector('english', "item"."name"), 'A') || setweight(to_tsvector('english', "item"."description"), 'B')) @@ (websearch_to_tsquery('english', $4)))
            GROUP BY "item"."id"
            HAVING COUNT(*) > $5 AND SUM("item"."price") > $6)
    "#
			.normalize()
	);
}

#[test]
fn web_payload_supports_nested_join_paths() {
	let json: Value = json!({
		"joins": [
			{ "path": ["material", "supplier"], "kind": "inner" }
		]
	});

	let f: WebFilter<ItemDto> =
		serde_json::from_value(json).expect("valid ItemDtoFilter");

	let sql = QueryBuilder::<Item>::from_dto::<ItemDto>(&f)
		.build()
		.sql(SelectType::Star)
		.trim_start()
		.normalize();

	assert_eq!(
		sql,
		r#"
        SELECT "item".*, "material__"."id" AS "__sqlxo_material__id",
            "material__"."name" AS "__sqlxo_material__name",
            "material__"."long_name" AS "__sqlxo_material__long_name",
            "material__"."description" AS "__sqlxo_material__description",
            "material__"."supplier_id" AS "__sqlxo_material__supplier_id",
            "material__supplier__"."id" AS "__sqlxo_material__supplier__id",
            "material__supplier__"."name" AS "__sqlxo_material__supplier__name"
        FROM item
        INNER JOIN material AS "material__" ON "item"."material_id" = "material__"."id"
        INNER JOIN supplier AS "material__supplier__" ON "material__"."supplier_id" = "material__supplier__"."id"
    "#
		.normalize()
	);
}
