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
            SELECT *
            FROM item
            LEFT JOIN material AS "material__" ON "item"."material_id" = "material__"."id"
            WHERE (name LIKE $1 AND (price > $2 OR description IS NULL))
            ORDER BY name ASC, price DESC
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
            SELECT *
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
