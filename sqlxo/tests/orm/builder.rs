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
	AppUser,
	AppUserJoin,
	CreateItem,
	CreateItemCreation,
	HardDeleteItem,
	Item,
	ItemJoin,
	ItemQuery,
	ItemSort,
	Material,
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
fn has_one_join_builds_sql() {
	let plan: ReadQueryPlan<AppUser> = QueryBuilder::<AppUser>::read()
		.join(AppUserJoin::AppUserToProfileByProfile, JoinKind::Left)
		.build();

	assert_eq!(
		plan.sql(SelectType::Star).trim_start().normalize(),
		r#"
            SELECT "app_user".*,
                "profile__"."id" AS "__sqlxo_profile__id",
                "profile__"."user_id" AS "__sqlxo_profile__user_id",
                "profile__"."bio" AS "__sqlxo_profile__bio"
            FROM app_user
            LEFT JOIN profile AS "profile__"
                ON "app_user"."id" = "profile__"."user_id"
        "#
		.normalize()
	);
}

#[test]
fn has_many_join_builds_sql() {
	let plan: ReadQueryPlan<Material> = QueryBuilder::<Material>::read()
		.join(MaterialJoin::MaterialToItemByItems, JoinKind::Left)
		.build();

	assert_eq!(
		plan.sql(SelectType::Star).trim_start().normalize(),
		r#"
            SELECT "material".*,
                "items__"."id" AS "__sqlxo_items__id",
                "items__"."name" AS "__sqlxo_items__name",
                "items__"."description" AS "__sqlxo_items__description",
                "items__"."price" AS "__sqlxo_items__price",
                "items__"."amount" AS "__sqlxo_items__amount",
                "items__"."active" AS "__sqlxo_items__active",
                "items__"."due_date" AS "__sqlxo_items__due_date",
                "items__"."material_id" AS "__sqlxo_items__material_id"
            FROM material
            LEFT JOIN item AS "items__"
                ON "material"."id" = "items__"."material_id"
        "#
		.normalize()
	);
}

#[test]
fn many_to_many_join_builds_sql() {
	let plan: ReadQueryPlan<Item> = QueryBuilder::<Item>::read()
		.join(ItemJoin::ItemToTagByTags, JoinKind::Left)
		.build();

	assert_eq!(
		plan.sql(SelectType::Star).trim_start().normalize(),
		r#"
            SELECT "item".*,
                "tags__"."id" AS "__sqlxo_tags__id",
                "tags__"."name" AS "__sqlxo_tags__name"
            FROM item
            LEFT JOIN item_tag AS "tags__pivot__"
                ON "item"."id" = "tags__pivot__"."item_id"
            LEFT JOIN tag AS "tags__"
                ON "tags__pivot__"."tag_id" = "tags__"."id"
        "#
		.normalize()
	);
}

#[test]
fn pivot_payload_join_builds_sql() {
	let plan: ReadQueryPlan<Item> = QueryBuilder::<Item>::read()
		.join(ItemJoin::ItemToItemTagByTagLinks, JoinKind::Left)
		.build();

	assert_eq!(
		plan.sql(SelectType::Star).trim_start().normalize(),
		r#"
            SELECT "item".*,
                "tag_links__"."id" AS "__sqlxo_tag_links__id",
                "tag_links__"."item_id" AS "__sqlxo_tag_links__item_id",
                "tag_links__"."tag_id" AS "__sqlxo_tag_links__tag_id",
                "tag_links__"."created_at" AS "__sqlxo_tag_links__created_at",
                "tag_links__"."note" AS "__sqlxo_tag_links__note"
            FROM item
            LEFT JOIN item_tag AS "tag_links__"
                ON "item"."id" = "tag_links__"."item_id"
        "#
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
