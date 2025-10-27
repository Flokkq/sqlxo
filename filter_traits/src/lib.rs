#![feature(trait_alias)]

use serde::{Deserialize, Serialize};
use sqlx::{prelude::Type, Postgres};
use utoipa::ToSchema;

pub trait QueryModel =
    Send + Clone + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + 'static;

pub trait QueryQuery = Filterable + Send + Clone + Sync;

pub trait QuerySort = Sortable + Send + Clone + Sync;

pub trait Filterable {
    type Entity: QueryModel;

    fn write<W: SqlWrite>(&self, w: &mut W);
}

pub trait SqlWrite {
    fn push(&mut self, s: &str);

    fn bind<T>(&mut self, value: T)
    where
        T: sqlx::Encode<'static, Postgres> + Send + 'static,
        T: Type<Postgres>;
}

pub trait QueryContext {
    const TABLE: &'static str;

    type Model: QueryModel;
    type Query: QueryQuery;
    type Sort: QuerySort;
    type Join: SqlJoin;
}

pub trait Sortable {
    type Entity: QueryModel;

    fn sort_clause(&self) -> String;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
    Left,
    Inner,
}

pub trait SqlJoin {
    fn to_sql(&self) -> String;

    fn kind(&self) -> JoinKind;
}

pub trait Model {}

pub trait WebQueryModel {
    type Leaf: ToSchema + Clone + serde::Serialize + for<'de> serde::Deserialize<'de>;
    type SortField: ToSchema + Clone + serde::Serialize + for<'de> serde::Deserialize<'de>;
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
#[serde(untagged)]
pub enum GenericDtoExpression<Q> {
    #[schema(no_recursion)]
    And {
        and: Vec<GenericDtoExpression<Q>>,
    },
    #[schema(no_recursion)]
    Or {
        or: Vec<GenericDtoExpression<Q>>,
    },
    Leaf(Q),
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
#[schema(bound = "S: ToSchema")]
#[serde(transparent)]
pub struct GenericDtoSort<S>(pub S);

#[derive(Clone, Copy, Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "lowercase")]
pub enum DtoSortDir {
    Asc,
    Desc,
}

#[derive(Clone, Copy, Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DtoPage {
    pub page_size: u32,
    pub page_no: u32,
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
#[schema(bound = "Q: ToSchema, S: ToSchema")]
pub struct GenericDtoFilter<Q, S> {
    #[schema(no_recursion)]
    pub filter: GenericDtoExpression<Q>,
    #[schema(no_recursion)]
    pub sort: Vec<GenericDtoSort<S>>,
    pub page: DtoPage,
}

pub type DtoExpression<T> = GenericDtoExpression<<T as WebQueryModel>::Leaf>;

pub type DtoSort<T> = GenericDtoSort<<T as WebQueryModel>::SortField>;

pub type DtoFilter<T> =
    GenericDtoFilter<<T as WebQueryModel>::Leaf, <T as WebQueryModel>::SortField>;
