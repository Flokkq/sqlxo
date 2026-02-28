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
	web::{
		WebDeleteFilter,
		WebReadFilter,
		WebUpdateFilter,
	},
	Buildable,
	JoinKind,
	QueryBuilder,
	ReadQueryPlan,
};
use uuid::Uuid;

use crate::helpers::{
	HardDeleteItem,
	HardDeleteItemDto,
	Item,
	ItemDto,
	ItemJoin,
	NormalizeString,
	UpdateItem,
	UpdateItemDto,
	UpdateItemUpdate,
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

	let f: WebReadFilter<ItemDto> =
		serde_json::from_value(json).expect("valid ItemDtoFilter");

	assert_some!(f.page);
	assert_eq!(f.page.unwrap().page_size, 10);
	assert_eq!(f.page.unwrap().page, 1);
}

#[test]
fn query_builder_from_web_query_filter() {
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

	let f: WebReadFilter<ItemDto> =
		serde_json::from_value(json).expect("valid ItemDtoFilter");

	let plan: ReadQueryPlan<Item> =
		QueryBuilder::<Item>::from_web_read::<ItemDto>(&f)
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
			{ "material": null }
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

	let f: WebReadFilter<ItemDto> =
		serde_json::from_value(json).expect("valid ItemDtoFilter");

	let plan: ReadQueryPlan<Item> =
		QueryBuilder::<Item>::from_web_read::<ItemDto>(&f).build();
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
			{ "material": [
				{ "supplier": null }
			]}
		]
	});

	let f: WebReadFilter<ItemDto> =
		serde_json::from_value(json).expect("valid ItemDtoFilter");

	let sql = QueryBuilder::<Item>::from_web_read::<ItemDto>(&f)
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
        LEFT JOIN material AS "material__" ON "item"."material_id" = "material__"."id"
        LEFT JOIN supplier AS "material__supplier__" ON "material__"."supplier_id" = "material__supplier__"."id"
    "#
		.normalize()
	);
}

#[test]
fn web_query_into_update_builds_sql() {
	let test_id = Uuid::new_v4();
	let json: Value = json!({
		"filter": { "id": { "eq": test_id } }
	});
	let filter: WebUpdateFilter<UpdateItemDto> =
		serde_json::from_value(json).expect("valid UpdateItemDto filter");

	let update = UpdateItemUpdate {
		name: Some("updated".into()),
		..Default::default()
	};

	let plan = QueryBuilder::<UpdateItem>::from_web_update::<
		UpdateItemDto,
	>(&filter)
	.model(update)
	.build();

	assert_eq!(
		plan.sql().normalize(),
		"UPDATE update_item SET updated_at = NOW(), name = $1 WHERE \
		 \"update_item\".\"id\" = $2"
	);
}

#[test]
fn web_query_into_delete_builds_sql() {
	let json: Value = json!({
		"filter": { "name": { "eq": "obsolete" } }
	});
	let filter: WebDeleteFilter<HardDeleteItemDto> =
		serde_json::from_value(json).expect("valid HardDeleteItemDto filter");

	let plan = QueryBuilder::<HardDeleteItem>::from_web_delete::<
		HardDeleteItemDto,
	>(&filter)
	.build();

	assert_eq!(
		plan.sql().normalize(),
		"DELETE FROM hard_delete_item WHERE \"hard_delete_item\".\"name\" = $1"
	);
}

#[test]
fn web_query_update_rejects_having() {
	let json: Value = json!({
		"having": { "count": { "gt": 1 } }
	});
	let result = serde_json::from_value::<WebUpdateFilter<UpdateItemDto>>(json);
	assert!(result.is_err());
}
