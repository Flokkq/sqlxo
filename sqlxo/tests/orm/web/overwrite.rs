use serde_json::{
	json,
	Value,
};
use sqlxo::{
	and,
	blocks::{
		BuildableFilter,
		BuildableSort,
		SelectType,
	},
	order_by,
	web::WebReadFilter,
	Buildable,
	QueryBuilder,
	ReadQueryPlan,
};

use crate::helpers::{
	Item,
	ItemDto,
	ItemQuery,
	ItemSort,
	NormalizeString,
};

#[test]
fn dto_filter_combined_with_inline_filter() {
	let json: Value = json!({
		"filter": {
			 "differentName": { "like": "%Sternlampe%" }
		},
		"sort": null,
		"page": null,
	});

	let f: WebReadFilter<ItemDto> =
		serde_json::from_value(json).expect("invalid ItemDtoFilter");

	let plan: ReadQueryPlan<Item> =
		QueryBuilder::<Item>::from_web_read::<ItemDto>(&f)
			.r#where(and![ItemQuery::NameIsNull, ItemQuery::AmountEq(1000)])
			.order_by(order_by![ItemSort::ByNameAsc])
			.build();

	assert_eq!(
		plan.sql(SelectType::Star).trim_start().normalize(),
		r#"
        SELECT "item".*
        FROM item
        WHERE ("item"."name" LIKE $1 AND ("item"."name" IS NULL AND "item"."amount" = $2)) ORDER BY "item"."name" ASC
    "#
		.normalize()
	);
}
