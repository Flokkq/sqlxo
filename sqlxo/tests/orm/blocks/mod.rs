use sqlxo::{
	and,
	blocks::{
		Expression,
		SortOrder,
	},
	or,
	order_by,
};

use crate::helpers::{
	ItemQuery,
	ItemSort,
};

#[test]
fn expression_macros() {
	let plain_query = Expression::Or(vec![
		Expression::And(vec![
			Expression::Leaf(ItemQuery::NameLike("%SternLampe%".into())),
			Expression::Leaf(ItemQuery::DescriptionNeq("Hohlweg".into())),
		]),
		Expression::Leaf(ItemQuery::PriceGt(1800f32)),
	]);

	let long_macro_query = or![
		and![
			Expression::Leaf(ItemQuery::NameLike("%SternLampe%".into())),
			Expression::Leaf(ItemQuery::DescriptionNeq("Hohlweg".into())),
		],
		Expression::Leaf(ItemQuery::PriceGt(1800f32)),
	];

	let short_macro_query = or![
		and![
			ItemQuery::NameLike("%SternLampe%".into()),
			ItemQuery::DescriptionNeq("Hohlweg".into()),
		],
		ItemQuery::PriceGt(1800f32),
	];

	assert_eq!(plain_query, long_macro_query);
	assert_eq!(long_macro_query, short_macro_query);
}

#[test]
fn sort_macros() {
	let plain_sort =
		SortOrder(vec![ItemSort::ByAmountAsc, ItemSort::ByNameDesc]);
	let short_macro_sort =
		order_by![ItemSort::ByAmountAsc, ItemSort::ByNameDesc];

	assert_eq!(plain_sort, short_macro_sort);
}
