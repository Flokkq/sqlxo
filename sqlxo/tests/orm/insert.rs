use crate::helpers::{
	CreateItem,
	CreateItemCreation,
	NormalizeString,
};
use sqlxo::{
	Buildable,
	Creatable,
	QueryBuilder,
};
use uuid::Uuid;

#[test]
fn test_create_struct_generated() {
	let create = CreateItemCreation {
		id:          Uuid::new_v4(),
		name:        "new item".into(),
		description: "new desc".into(),
		price:       99.99,
	};

	assert_eq!(create.name, "new item");
	assert_eq!(create.description, "new desc");
	assert_eq!(create.price, 99.99);
}

#[test]
fn test_create_derives_creatable() {
	assert_eq!(CreateItem::INSERT_MARKER_FIELD, Some("created_at"));
}

#[cfg(any(test, feature = "test-utils"))]
#[test]
fn test_insert_sql_exact_match() {
	let test_id = Uuid::new_v4();

	let create = CreateItemCreation {
		id:          test_id,
		name:        "test".into(),
		description: "desc".into(),
		price:       99.99,
	};

	let plan = QueryBuilder::<CreateItem>::insert().model(create).build();

	assert_eq!(
		plan.sql().normalize(),
		"INSERT INTO create_item (id, name, description, price, created_at) \
		 VALUES ($1, $2, $3, $4, NOW())"
	);
}
