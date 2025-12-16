use crate::helpers::{
	NormalizeString,
	UpdateItem,
	UpdateItemQuery,
	UpdateItemUpdate,
};
use sqlxo::blocks::{
	BuildableFilter,
	Expression,
};
use sqlxo::{
	Buildable,
	QueryBuilder,
	Updatable,
};
use uuid::Uuid;

#[test]
fn test_update_struct_generated() {
	let update = UpdateItemUpdate {
		name:        Some("new name".into()),
		description: Some("new desc".into()),
		price:       Some(99.99),
	};

	assert!(update.name.is_some());
	assert!(update.description.is_some());
	assert!(update.price.is_some());
}

#[test]
fn test_update_default() {
	let update = UpdateItemUpdate::default();

	assert!(update.name.is_none());
	assert!(update.description.is_none());
	assert!(update.price.is_none());
}

#[test]
fn test_update_derives_updatable() {
	assert_eq!(UpdateItem::UPDATE_MARKER_FIELD, Some("updated_at"));
}

#[test]
fn test_update_partial_fields() {
	let update = UpdateItemUpdate {
		name:        Some("only name".into()),
		description: None,
		price:       None,
	};

	assert!(update.name.is_some());
	assert!(update.description.is_none());
	assert!(update.price.is_none());
}

#[cfg(any(test, feature = "test-utils"))]
#[test]
fn test_update_sql_exact_match_single_field() {
	let test_id = Uuid::new_v4();

	let update = UpdateItemUpdate {
		name: Some("test".into()),
		..Default::default()
	};

	let plan = QueryBuilder::<UpdateItem>::update()
		.model(update)
		.r#where(Expression::Leaf(UpdateItemQuery::IdEq(test_id)))
		.build();

	assert_eq!(
		plan.sql().normalize(),
		"UPDATE update_item SET updated_at = NOW(), name = $1 WHERE id = $2"
	);
}

#[cfg(any(test, feature = "test-utils"))]
#[test]
fn test_update_sql_exact_match_multiple_fields() {
	let test_id = Uuid::new_v4();

	let update = UpdateItemUpdate {
		name:        Some("test".into()),
		description: Some("desc".into()),
		price:       Some(99.99),
	};

	let plan = QueryBuilder::<UpdateItem>::update()
		.model(update)
		.r#where(Expression::Leaf(UpdateItemQuery::IdEq(test_id)))
		.build();

	assert_eq!(
		plan.sql().normalize(),
		"UPDATE update_item SET updated_at = NOW(), name = $1, description = \
		 $2, price = $3 WHERE id = $4"
	);
}

#[cfg(any(test, feature = "test-utils"))]
#[test]
fn test_update_sql_no_marker_just_fields() {
	let test_id = Uuid::new_v4();

	let update = UpdateItemUpdate {
		name:        Some("test".into()),
		description: Some("desc".into()),
		price:       None,
	};

	let plan = QueryBuilder::<UpdateItem>::update()
		.model(update)
		.r#where(Expression::Leaf(UpdateItemQuery::IdEq(test_id)))
		.build();

	assert_eq!(
		plan.sql().normalize(),
		"UPDATE update_item SET updated_at = NOW(), name = $1, description = \
		 $2 WHERE id = $3"
	);
}
