use crate::ItemQuery::*;
use crate::ItemSort::*;
use filter_macros::Query;
use filter_traits::Filterable;
use filter_traits::QueryContext;
use sqlx::Execute;
use sqlx::FromRow;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, Clone, FromRow, Query)]
#[filter(table_name = "item")]
pub struct Item {
    id: Uuid,
    name: String,
    description: String,
    price: f32,
    amount: i32,
    active: bool,
    due_date: chrono::DateTime<chrono::Utc>,
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
    let _ = DueDateOn(now);
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
fn query_enum_generates_expected_sql() {
    let mut idx = 0;
    assert_eq!(
        ItemQuery::NameEq("foo".into()).filter_clause(&mut idx),
        "name = $1"
    );
    assert_eq!(idx, 1);

    assert_eq!(
        ItemQuery::PriceGt(42.0).filter_clause(&mut idx),
        "price > $2"
    );
    assert_eq!(idx, 2);

    assert_eq!(
        ItemQuery::ActiveIsTrue.filter_clause(&mut idx),
        "active = TRUE"
    );
    assert_eq!(idx, 2, "no new bind parameter for pure boolean expr");
}

#[test]
fn query_enum_full_query_matches_handwritten() {
    let mut idx = 0;
    let clause = ItemQuery::NameEq("foo".into()).filter_clause(&mut idx);
    let generated = format!("SELECT * FROM item WHERE ({clause})");

    let handwritten = "SELECT * FROM item WHERE (name = $1)";

    assert_eq!(generated, handwritten);
}

use sqlx::{postgres::PgArguments, Arguments};

#[test]
fn bind_adds_expected_number_of_args_for_string_eq() {
    let q = sqlx::query_as::<_, Item>("SELECT * FROM item WHERE name = $1");
    let q = ItemQuery::NameEq("foo".into()).bind(q);

    let mut q = q;
    let args: PgArguments = q.take_arguments().expect("arguments present").unwrap();
    assert_eq!(args.len(), 1, "NameEq should bind 1 argument");
}

#[test]
fn bind_adds_expected_number_of_args_for_between() {
    let q = sqlx::query_as::<_, Item>("SELECT * FROM item WHERE price BETWEEN $1 AND $2");
    let q = ItemQuery::PriceBetween(10.0_f32, 99.0_f32).bind(q);

    let mut q = q;
    let args: PgArguments = q.take_arguments().expect("arguments present").unwrap();
    assert_eq!(args.len(), 2, "PriceBetween should bind 2 arguments");
}

#[test]
fn bind_for_bool_ops_binds_nothing() {
    let q = sqlx::query_as::<_, Item>("SELECT * FROM item WHERE active = TRUE");
    let q = ItemQuery::ActiveIsTrue.bind(q);

    let mut q = q;
    let args: PgArguments = q.take_arguments().unwrap_or_default().unwrap();
    assert_eq!(args.len(), 0, "ActiveIsTrue should bind no arguments");
}

#[test]
fn bind_chain_preserves_order_and_sql() {
    let sql = "SELECT * FROM item WHERE name = $1 AND price > $2 AND active = TRUE";
    let q = sqlx::query_as::<_, Item>(sql);

    let q = ItemQuery::NameEq("foo".into()).bind(q);
    let q = ItemQuery::PriceGt(42.0_f32).bind(q);
    let q = ItemQuery::ActiveIsTrue.bind(q);

    assert_eq!(q.sql(), sql);

    let mut q = q;
    let args: PgArguments = q.take_arguments().expect("arguments present").unwrap();
    assert_eq!(
        args.len(),
        2,
        "two arguments should be bound for NameEq and PriceGt"
    );
}

#[test]
fn bind_for_uuid_and_datetime() {
    let uid = Uuid::nil();
    let now = chrono::Utc::now();

    let q = sqlx::query_as::<_, Item>("SELECT * FROM item WHERE id = $1");
    let q = ItemQuery::IdEq(uid).bind(q);
    let mut q = q;
    let args: PgArguments = q.take_arguments().expect("arguments present").unwrap();
    assert_eq!(args.len(), 1, "IdEq should bind 1 argument");

    let q = sqlx::query_as::<_, Item>("SELECT * FROM item WHERE due_date BETWEEN $1 AND $2");
    let q = ItemQuery::DueDateBetween(now, now).bind(q);
    let mut q = q;
    let args: PgArguments = q.take_arguments().expect("arguments present").unwrap();
    assert_eq!(args.len(), 2, "DueDateBetween should bind 2 arguments");
}
