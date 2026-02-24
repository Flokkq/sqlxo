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
	DeleteQueryPlan,
	InsertQueryPlan,
	JoinKind,
	QueryBuilder,
	ReadQueryPlan,
	UpdateQueryPlan,
};
use uuid::Uuid;

use crate::helpers::{
	CreateItem,
	CreateItemCreation,
	HardDeleteItem,
	Item,
	ItemJoin,
	ItemQuery,
	ItemSort,
	MaterialJoin,
	NormalizeString,
	UpdateItem,
	UpdateItemUpdate,
};

#[test]
fn query_builder() {
	let plan: ReadQueryPlan<Item> = QueryBuilder::read()
		.join(ItemJoin::ItemToMaterialByMaterialId, JoinKind::Left)
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
            SELECT "item".*, "material__"."id" AS "__sqlxo_material__id",
                "material__"."name" AS "__sqlxo_material__name",
                "material__"."long_name" AS "__sqlxo_material__long_name",
                "material__"."description" AS "__sqlxo_material__description",
                "material__"."supplier_id" AS "__sqlxo_material__supplier_id"
            FROM item
            LEFT JOIN material AS "material__" ON "item"."material_id" = "material__"."id"
            WHERE ("item"."name" LIKE $1 AND ("item"."price" > $2 OR "item"."description" IS NULL))
            ORDER BY "item"."name" ASC, "item"."price" DESC
            LIMIT $3 OFFSET $4
        "#
		.normalize()
	)
}

#[test]
fn nested_join_path_builds_sql() {
	let path = ItemJoin::ItemToMaterialByMaterialId
		.path(JoinKind::Left)
		.then(
			MaterialJoin::MaterialToSupplierBySupplierId,
			JoinKind::Inner,
		);

	let plan: ReadQueryPlan<Item> =
		QueryBuilder::read().join_path(path).build();

	assert_eq!(
		plan.sql(SelectType::Star).trim_start().normalize(),
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
            INNER JOIN supplier AS "material__supplier__" ON "material__"."supplier_id" = "material__supplier__"."id"
        "#
		.normalize()
	);
}

#[test]
fn read_builder_allows_custom_row_type() {
	let plan: ReadQueryPlan<Item, (Uuid,)> = QueryBuilder::<Item>::read()
		.take(sqlxo::take!(crate::helpers::ItemColumn::Id))
		.build();

	assert_eq!(
		plan.sql(SelectType::Star).normalize(),
		r#"SELECT "item"."id" FROM item"#.normalize()
	);
}

#[test]
fn read_builder_allows_aggregate_take() {
	let plan: ReadQueryPlan<Item, (i64,)> = QueryBuilder::<Item>::read()
		.take(sqlxo::take!(crate::helpers::ItemAgg::CountAll()))
		.build();

	assert_eq!(
		plan.sql(SelectType::Star).normalize(),
		r#"SELECT COUNT(*) AS "__sqlxo_sel_0" FROM item"#.normalize()
	);
}

#[test]
fn read_builder_allows_mixed_take() {
	let plan: ReadQueryPlan<Item, (String, Option<f32>, i64)> =
		QueryBuilder::<Item>::read()
			.take(sqlxo::take!(
				crate::helpers::ItemColumn::Name,
				crate::helpers::ItemAgg::Sum(crate::helpers::ItemColumn::Price),
				crate::helpers::ItemAgg::CountAll()
			))
			.group_by(sqlxo::group_by!(crate::helpers::ItemColumn::Name))
			.build();

	assert_eq!(
		plan.sql(SelectType::Star).normalize(),
		r#"SELECT "item"."name", SUM("item"."price") AS "__sqlxo_sel_1", COUNT(*) AS "__sqlxo_sel_2" FROM item GROUP BY "item"."name""#
			.normalize()
	);
}

#[test]
fn read_builder_allows_group_by_clause() {
	let plan: ReadQueryPlan<Item, (Option<Uuid>, i64)> =
		QueryBuilder::<Item>::read()
			.take(sqlxo::take!(
				crate::helpers::ItemColumn::MaterialId,
				crate::helpers::ItemAgg::CountAll()
			))
			.group_by(sqlxo::group_by!(crate::helpers::ItemColumn::MaterialId))
			.build();

	assert_eq!(
		plan.sql(SelectType::Star).normalize(),
		r#"SELECT "item"."material_id", COUNT(*) AS "__sqlxo_sel_1" FROM item GROUP BY "item"."material_id""#
			.normalize()
	);
}

#[test]
fn read_builder_allows_having_clause() {
	let plan: ReadQueryPlan<Item, (i64,)> = QueryBuilder::<Item>::read()
		.take(sqlxo::take!(crate::helpers::ItemAgg::CountAll()))
		.having(sqlxo::having!(crate::helpers::ItemAgg::CountAll().gt(5i64)))
		.build();

	assert_eq!(
		plan.sql(SelectType::Star).normalize(),
		r#"SELECT COUNT(*) AS "__sqlxo_sel_0" FROM item HAVING COUNT(*) > $1"#
			.normalize()
	);
}

#[test]
fn delete_builder_allows_custom_row_type() {
	let plan: DeleteQueryPlan<HardDeleteItem, (Uuid,)> =
		QueryBuilder::<HardDeleteItem>::delete()
			.take(sqlxo::take!(crate::helpers::HardDeleteItemColumn::Id))
			.build();

	assert_eq!(plan.sql().normalize(), "DELETE FROM hard_delete_item");
}

#[test]
fn insert_builder_allows_custom_row_type() {
	let create = CreateItemCreation {
		id:          Uuid::new_v4(),
		name:        "builder insert".into(),
		description: "row subset".into(),
		price:       10.0,
	};
	let plan: InsertQueryPlan<CreateItem, (Uuid,)> =
		QueryBuilder::<CreateItem>::insert()
			.model(create)
			.take(sqlxo::take!(crate::helpers::CreateItemColumn::Id))
			.build();

	assert_eq!(
		plan.sql().normalize(),
		"INSERT INTO create_item (id, name, description, price, created_at) \
		 VALUES ($1, $2, $3, $4, NOW())"
	);
}

#[test]
fn update_builder_allows_custom_row_type() {
	let update = UpdateItemUpdate {
		name:        Some("builder update".into()),
		description: None,
		price:       None,
	};
	let plan: UpdateQueryPlan<UpdateItem, (Uuid, String)> =
		QueryBuilder::<UpdateItem>::update()
			.model(update)
			.take(sqlxo::take!(
				crate::helpers::UpdateItemColumn::Id,
				crate::helpers::UpdateItemColumn::Name
			))
			.build();

	assert_eq!(
		plan.sql().normalize(),
		"UPDATE update_item SET updated_at = NOW(), name = $1"
	);
}

#[test]
fn read_builder_supports_take_with_join_columns() {
	let plan: ReadQueryPlan<Item, (Uuid, Uuid)> = QueryBuilder::<Item>::read()
		.join(ItemJoin::ItemToMaterialByMaterialId, JoinKind::Inner)
		.take(sqlxo::take!(
			crate::helpers::ItemColumn::Id,
			crate::helpers::MaterialColumn::Id
		))
		.build();

	assert_eq!(
		plan.sql(SelectType::Star).trim_start().normalize(),
		r#"
            SELECT "item"."id", "material__"."id"
            FROM item
            INNER JOIN material AS "material__" ON "item"."material_id" = "material__"."id"
        "#
		.normalize()
	);
}
