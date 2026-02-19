#![cfg(test)]

use chrono::{
	DateTime,
	Utc,
};
use sqlx::FromRow;
use sqlxo::{
	Buildable,
	Delete,
	Query,
	QueryBuilder,
	SoftDelete,
};
use uuid::Uuid;
use crate::helpers::NormalizeString;

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow, Query, Delete)]
#[sqlxo(table_name = "hard_item")]
pub struct HardDeleteItem {
	#[primary_key]
	pub id:          Uuid,
	pub name:        String,
	pub description: Option<String>,
	pub due_date:    DateTime<Utc>,
	pub created_at:  DateTime<Utc>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow, Query, SoftDelete)]
#[sqlxo(table_name = "soft_item")]
pub struct SoftDeleteItem {
	#[primary_key]
	pub id:          Uuid,
	pub name:        String,
	pub description: Option<String>,
	pub due_date:    DateTime<Utc>,
	pub created_at:  DateTime<Utc>,
	#[sqlxo(delete_marker)]
	pub deleted_at:  Option<DateTime<Utc>>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow, Query, Delete)]
pub struct ItemWithCascade {
	#[primary_key]
	pub id:          Uuid,
	pub name:        String,
	#[foreign_key(to = "user.user_id", cascade_type(cascade))]
	pub user_id:     Uuid,
	#[foreign_key(to = "category.id", cascade_type(restrict))]
	pub category_id: Option<Uuid>,
	#[foreign_key(to = "parent.id", cascade_type(set_null))]
	pub parent_id:   Option<Uuid>,
}

#[test]
fn test_hard_delete_derives() {
	use sqlxo::Deletable;
	assert_eq!(HardDeleteItem::IS_SOFT_DELETE, false);
	assert_eq!(HardDeleteItem::DELETE_MARKER_FIELD, None);
}

#[test]
fn test_soft_delete_derives() {
	use sqlxo::Deletable;
	assert_eq!(SoftDeleteItem::IS_SOFT_DELETE, true);
	assert_eq!(SoftDeleteItem::DELETE_MARKER_FIELD, Some("deleted_at"));
}

#[cfg(any(test, feature = "test-utils"))]
#[test]
fn test_hard_delete_sql_generation() {
	use sqlxo::blocks::{
		BuildableFilter,
		Expression,
	};

	let plan = QueryBuilder::<HardDeleteItem>::delete()
		.r#where(Expression::Leaf(HardDeleteItemQuery::NameEq("test".into())))
		.build();

	let sql = plan.sql().normalize();
	assert_eq!(sql, "DELETE FROM hard_item WHERE name = $1");
}

#[cfg(any(test, feature = "test-utils"))]
#[test]
fn test_soft_delete_sql_generation() {
	use sqlxo::blocks::{
		BuildableFilter,
		Expression,
	};

	let plan = QueryBuilder::<SoftDeleteItem>::delete()
		.r#where(Expression::Leaf(SoftDeleteItemQuery::NameEq("test".into())))
		.build();

	let sql = plan.sql().normalize();
	assert_eq!(sql, "UPDATE soft_item SET deleted_at = NOW() WHERE name = $1");
}

#[cfg(any(test, feature = "test-utils"))]
#[test]
fn test_read_excludes_soft_deleted() {
	use sqlxo::blocks::{
		BuildableFilter,
		Expression,
		SelectType,
	};

	// Without include_deleted, soft-deleted records should be filtered out
	let plan = QueryBuilder::<SoftDeleteItem>::read()
		.r#where(Expression::Leaf(SoftDeleteItemQuery::NameEq("test".into())))
		.build();

	let sql = plan.sql(SelectType::Star).normalize();
	assert_eq!(
		sql,
		"SELECT * FROM soft_item WHERE deleted_at IS NULL AND (name = $1)"
	);
}

#[cfg(any(test, feature = "test-utils"))]
#[test]
fn test_read_includes_soft_deleted_when_requested() {
	use sqlxo::blocks::{
		BuildableFilter,
		Expression,
		SelectType,
	};

	let plan = QueryBuilder::<SoftDeleteItem>::read()
		.include_deleted()
		.r#where(Expression::Leaf(SoftDeleteItemQuery::NameEq("test".into())))
		.build();

	let sql = plan.sql(SelectType::Star).normalize();
	assert_eq!(sql, "SELECT * FROM soft_item WHERE name = $1");
}

#[cfg(any(test, feature = "test-utils"))]
#[test]
fn test_read_hard_delete_no_filter() {
	use sqlxo::blocks::{
		BuildableFilter,
		Expression,
		SelectType,
	};

	let plan = QueryBuilder::<HardDeleteItem>::read()
		.r#where(Expression::Leaf(HardDeleteItemQuery::NameEq("test".into())))
		.build();

	let sql = plan.sql(SelectType::Star).normalize();
	assert_eq!(sql, "SELECT * FROM hard_item WHERE name = $1");
}

#[cfg(any(test, feature = "test-utils"))]
#[test]
fn test_soft_delete_without_where() {
	let plan = QueryBuilder::<SoftDeleteItem>::delete().build();

	let sql = plan.sql().normalize();
	assert_eq!(sql, "UPDATE soft_item SET deleted_at = NOW()");
}

#[cfg(any(test, feature = "test-utils"))]
#[test]
fn test_read_soft_delete_without_where() {
	use sqlxo::blocks::SelectType;

	let plan = QueryBuilder::<SoftDeleteItem>::read().build();

	let sql = plan.sql(SelectType::Star).normalize();
	assert_eq!(sql, "SELECT * FROM soft_item WHERE deleted_at IS NULL");
}
