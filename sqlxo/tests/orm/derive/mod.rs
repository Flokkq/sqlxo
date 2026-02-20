use sqlxo_traits::Filterable;
use sqlxo_traits::QueryContext;
use sqlxo_traits::Sortable;
use sqlxo_traits::SqlWrite;
use uuid::Uuid;

use crate::helpers::Item;
use crate::helpers::ItemQuery;
use crate::helpers::ItemQuery::*;
use crate::helpers::ItemSort::*;

mod trybuild;

#[derive(Default, Debug)]
pub struct DummyWriter {
	sql:   String,
	binds: usize,
}

impl SqlWrite for DummyWriter {
	fn push(&mut self, s: &str) {
		self.sql.push_str(&s);
	}

	fn bind<T>(&mut self, _value: T)
	where
		T: sqlx::Encode<'static, sqlx::Postgres> + Send + 'static,
		T: sqlx::Type<sqlx::Postgres>,
	{
		self.binds += 1;

		use std::fmt::Write as _;
		let _ = write!(&mut self.sql, "${}", self.binds);
	}
}

fn assert_write(q: ItemQuery, expected_sql: &str, expcted_binds: usize) {
	let mut w = DummyWriter::default();

	q.write(&mut w);

	assert_eq!(w.sql, expected_sql, "sql missmatch");
	assert_eq!(w.binds, expcted_binds, "bind count missmatch");
}

#[test]
fn table_const() {
	assert_eq!(<Item as QueryContext>::TABLE, "item");
}

#[test]
fn query_enum_has_all_expected_string_ops() {
	let _ = NameEq("Lamp".into());
	let _ = NameNeq("X".into());
	let _ = NameLike("%stern%".into());
	let _ = NameNotLike("%broken%".into());
	let _ = NameIsNull;
	let _ = NameIsNotNull;

	let _ = DescriptionLike("%von Hohberg%".into());
	let _ = DescriptionIsNull;
	let _ = DescriptionIsNotNull;
}

#[test]
fn query_enum_has_all_expected_number_ops() {
	let _ = PriceEq(9.9_f32);
	let _ = PriceNeq(9.9_f32);
	let _ = PriceGt(10.0_f32);
	let _ = PriceGte(10.0_f32);
	let _ = PriceLt(99.0_f32);
	let _ = PriceLte(99.0_f32);
	let _ = PriceBetween(10.0_f32, 99.0_f32);
	let _ = PriceNotBetween(10.0_f32, 99.0_f32);

	let _ = AmountEq(1_i32);
	let _ = AmountBetween(1_i32, 10_i32);
	let _ = AmountNotBetween(1_i32, 10_i32);
}

#[test]
fn query_enum_has_bool_ops() {
	let _ = ActiveIsTrue;
	let _ = ActiveIsFalse;
}

#[test]
fn query_enum_has_uuid_ops_and_datetime_ops() {
	let _ = IdEq(Uuid::nil());
	let _ = IdNeq(Uuid::nil());
	let _ = IdIsNull;
	let _ = IdIsNotNull;

	let now = chrono::Utc::now();
	let _ = DueDateEq(now);
	let _ = DueDateBetween(now, now);
	let _ = DueDateIsNull;
	let _ = DueDateIsNotNull;
}

#[test]
fn sort_enum_variants_exist() {
	let _ = ByNameAsc;
	let _ = ByNameDesc;
	let _ = ByPriceAsc;
	let _ = ByPriceDesc;
	let _ = ByAmountAsc;
	let _ = ByAmountDesc;
	let _ = ByActiveAsc;
	let _ = ByActiveDesc;
	let _ = ByDueDateAsc;
	let _ = ByDueDateDesc;
	let _ = ByIdAsc;
	let _ = ByIdDesc;
}

#[test]
fn string_ops_write_expected_sql() {
	assert_write(NameEq("foo".into()), r#""item"."name" = $1"#, 1);
	assert_write(NameNeq("bar".into()), r#""item"."name" <> $1"#, 1);
	assert_write(NameLike("%x%".into()), r#""item"."name" LIKE $1"#, 1);
	assert_write(NameNotLike("%x%".into()), r#""item"."name" NOT LIKE $1"#, 1);
	assert_write(DescriptionIsNull, r#""item"."description" IS NULL"#, 0);
	assert_write(
		DescriptionIsNotNull,
		r#""item"."description" IS NOT NULL"#,
		0,
	);
}

#[test]
fn bool_ops_write_expected_sql() {
	assert_write(ActiveIsTrue, r#""item"."active" = TRUE"#, 0);
	assert_write(ActiveIsFalse, r#""item"."active" = FALSE"#, 0);
}

#[test]
fn numeric_ops_write_expected_sql_and_binds() {
	assert_write(PriceEq(1.5), r#""item"."price" = $1"#, 1);
	assert_write(PriceNeq(1.5), r#""item"."price" <> $1"#, 1);
	assert_write(PriceGt(2.0), r#""item"."price" > $1"#, 1);
	assert_write(PriceGte(2.0), r#""item"."price" >= $1"#, 1);
	assert_write(PriceLt(2.0), r#""item"."price" < $1"#, 1);
	assert_write(PriceLte(2.0), r#""item"."price" <= $1"#, 1);
	assert_write(
		PriceBetween(10.0, 99.0),
		r#""item"."price" BETWEEN $1 AND $2"#,
		2,
	);
	assert_write(
		PriceNotBetween(10.0, 99.0),
		r#""item"."price" NOT BETWEEN $1 AND $2"#,
		2,
	);

	assert_write(AmountGt(5), r#""item"."amount" > $1"#, 1);
}

#[test]
fn uuid_ops_write_expected_sql() {
	let uid = Uuid::default();
	let mut w = DummyWriter::default();

	IdEq(uid).write(&mut w);
	assert_eq!(w.sql, r#""item"."id" = $1"#);
	assert_eq!(w.binds, 1);

	let mut w = DummyWriter::default();
	IdNeq(uid).write(&mut w);
	assert_eq!(w.sql, r#""item"."id" <> $1"#);
	assert_eq!(w.binds, 1);

	assert_write(IdIsNull, r#""item"."id" IS NULL"#, 0);
	assert_write(IdIsNotNull, r#""item"."id" IS NOT NULL"#, 0);
}

#[test]
fn datetime_ops_write_expected_sql() {
	use sqlx::types::chrono::{
		DateTime,
		Utc,
	};
	let now: DateTime<Utc> = Utc::now();

	let mut w = DummyWriter::default();
	DueDateEq(now).write(&mut w);
	assert_eq!(w.sql, r#""item"."due_date" = $1"#);
	assert_eq!(w.binds, 1);

	let mut w = DummyWriter::default();
	DueDateBetween(now, now).write(&mut w);
	assert_eq!(w.sql, r#""item"."due_date" BETWEEN $1 AND $2"#);
	assert_eq!(w.binds, 2);

	assert_write(DueDateIsNull, r#""item"."due_date" IS NULL"#, 0);
	assert_write(DueDateIsNotNull, r#""item"."due_date" IS NOT NULL"#, 0);
}

#[test]
fn sort_variants_emit_expected_clauses() {
	assert_eq!(ByNameAsc.sort_clause(), r#""item"."name" ASC"#);
	assert_eq!(ByNameDesc.sort_clause(), r#""item"."name" DESC"#);

	assert_eq!(ByPriceAsc.sort_clause(), r#""item"."price" ASC"#);
	assert_eq!(ByPriceDesc.sort_clause(), r#""item"."price" DESC"#);

	assert_eq!(ByAmountAsc.sort_clause(), r#""item"."amount" ASC"#);
	assert_eq!(ByAmountDesc.sort_clause(), r#""item"."amount" DESC"#);

	assert_eq!(ByActiveAsc.sort_clause(), r#""item"."active" ASC"#);
	assert_eq!(ByActiveDesc.sort_clause(), r#""item"."active" DESC"#);

	assert_eq!(ByDueDateAsc.sort_clause(), r#""item"."due_date" ASC"#);
	assert_eq!(ByDueDateDesc.sort_clause(), r#""item"."due_date" DESC"#);
}
