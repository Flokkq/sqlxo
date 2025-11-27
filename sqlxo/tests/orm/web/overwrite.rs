use serde_json::{
	json,
	Value,
};
use sqlxo::{
	and,
	blocks::BuildType,
	order_by,
	web::WebFilter,
	QueryBuilder,
	QueryPlan,
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

	let plan: QueryPlan<Item> = QueryBuilder::<Item>::from_dto::<ItemDto>(&f)
		.r#where(and![ItemQuery::NameIsNull, ItemQuery::AmountEq(1000)])
		.order_by(order_by![ItemSort::ByNameAsc])
		.build();

	assert_eq!(
		plan.sql(BuildType::Raw).trim_start().normalize(),
		r#"
        WHERE (name LIKE $1 AND (name IS NULL AND amount = $2)) ORDER BY name ASC
    "#
		.normalize()
	);
}
