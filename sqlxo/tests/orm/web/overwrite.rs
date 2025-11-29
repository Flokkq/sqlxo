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
	web::WebFilter,
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
			 "different_name": { "like": "%Sternlampe%" }
		},
		"sort": null,
		"page": null,
	});

	let f: WebFilter<ItemDto> =
		serde_json::from_value(json).expect("invalid ItemDtoFilter");

	let plan: ReadQueryPlan<Item> =
		QueryBuilder::<Item>::from_dto::<ItemDto>(&f)
			.r#where(and![ItemQuery::NameIsNull, ItemQuery::AmountEq(1000)])
			.order_by(order_by![ItemSort::ByNameAsc])
			.build();

	assert_eq!(
		plan.sql(SelectType::Star).trim_start().normalize(),
		r#"
        SELECT *
        FROM item
        WHERE (name LIKE $1 AND (name IS NULL AND amount = $2)) ORDER BY name ASC
    "#
		.normalize()
	);
}
