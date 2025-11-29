use sqlxo::{
	and,
	blocks::{
		BuildableFilter,
		BuildableJoin,
		BuildablePage,
		BuildableSort,
		Pagination,
		SelectType,
	},
	or,
	order_by,
	Buildable,
	JoinKind,
	QueryBuilder,
	ReadQueryPlan,
};

use crate::helpers::{
	Item,
	ItemJoin,
	ItemQuery,
	ItemSort,
	NormalizeString,
};

#[test]
fn query_builder() {
	let plan: ReadQueryPlan<Item> = QueryBuilder::insert()
		.join(ItemJoin::ItemToMaterialByMaterialId(JoinKind::Left))
		.r#where(and![ItemQuery::NameLike("Clemens".into()), or![
			ItemQuery::PriceGt(1800.00f32),
			ItemQuery::DescriptionIsNull,
		]])
		.order_by(order_by![ItemSort::ByNameAsc, ItemSort::ByPriceDesc])
		.paginate(Pagination {
			page:      2,
			page_size: 50,
		})
		.build();

	assert_eq!(
		plan.sql(SelectType::Star).trim_start(),
		r#"
            SELECT *
            FROM item
            LEFT JOIN material ON "item"."material_id" = "material"."id"
            WHERE (name LIKE $1 AND (price > $2 OR description IS NULL))
            ORDER BY name ASC, price DESC
            LIMIT $3 OFFSET $4
        "#
		.normalize()
	)
}
