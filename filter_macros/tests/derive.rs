use crate::ItemQuery::*;
use crate::ItemSort::*;
use filter_macros::Query;
use sqlx::FromRow;
use uuid::Uuid;

#[allow(dead_code)]
#[derive(Debug, FromRow, Query)]
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
    assert_eq!(ITEM_TABLE, "item");
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
