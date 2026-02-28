#![feature(trait_alias)]
#![forbid(unsafe_code)]
#![feature(specialization)]
#![feature(inherent_associated_types)]
#![allow(incomplete_features)]
extern crate self as sqlxo;

pub use smallvec;
pub use sqlxo_macros::*;
pub use sqlxo_traits::*;

pub mod prelude {
	pub use super::{
		Filterable,
		JoinKind,
		Query,
		QueryContext,
		Sortable,
		SqlJoin,
		WebQuery,
	};
}

pub mod blocks;
pub mod fts;
pub mod select;
pub mod web;

mod delete;
mod insert;
mod read;
mod update;
pub use delete::{
	DeleteQueryBuilder,
	DeleteQueryPlan,
};
pub use insert::{
	InsertQueryBuilder,
	InsertQueryPlan,
};
pub use read::{
	ReadQueryBuilder,
	ReadQueryPlan,
};
pub use select::{
	Column,
	SelectionList,
};
pub use update::{
	UpdateQueryBuilder,
	UpdateQueryPlan,
};

use sqlx::{
	Executor,
	Postgres,
};

pub struct QueryBuilder<C> {
	_phantom: std::marker::PhantomData<C>,
}

impl<'a, C> QueryBuilder<C>
where
	C: QueryContext,
{
	pub fn read() -> ReadQueryBuilder<'a, C> {
		ReadQueryBuilder::from_ctx()
	}

	pub fn delete() -> DeleteQueryBuilder<'a, C>
	where
		C::Model: crate::Deletable,
	{
		DeleteQueryBuilder::from_ctx()
	}

	pub fn update() -> UpdateQueryBuilder<'a, C>
	where
		C::Model: crate::Updatable,
	{
		UpdateQueryBuilder::from_ctx()
	}

	pub fn insert() -> InsertQueryBuilder<'a, C>
	where
		C::Model: crate::Creatable,
	{
		InsertQueryBuilder::from_ctx()
	}
}

#[async_trait::async_trait]
pub trait ExecutablePlan<C: QueryContext> {
	async fn execute<'e, E>(&self, exec: E) -> Result<u64, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>;
}

#[async_trait::async_trait]
pub trait FetchablePlan<C: QueryContext, Row> {
	async fn fetch_one<'e, E>(&self, exec: E) -> Result<Row, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>;

	async fn fetch_all<'e, E>(&self, exec: E) -> Result<Vec<Row>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>;

	async fn fetch_optional<'e, E>(
		&self,
		exec: E,
	) -> Result<Option<Row>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>;
}

pub trait Planable<C, Row>: ExecutablePlan<C> + FetchablePlan<C, Row>
where
	C: QueryContext,
{
}

pub trait Buildable<C: QueryContext> {
	type Row;
	type Plan: Planable<C, Self::Row>;

	fn from_ctx() -> Self;
	fn build(self) -> Self::Plan;
}

#[macro_export]
macro_rules! web_query_schema {
	($model:ty) => {
		$crate::web::GenericWebExpression<
			<$model as $crate::WebQueryModel>::Leaf
		>,
		$crate::web::GenericWebSort<
			<$model as $crate::WebQueryModel>::SortField
		>,
		$crate::web::GenericWebFilter<
			<$model as $crate::WebQueryModel>::Leaf,
			<$model as $crate::WebQueryModel>::SortField,
			<$model as $crate::WebQueryModel>::AggregateLeaf,
			<$model as $crate::WebQueryModel>::JoinPath,
		>
	};
	($model:ty, $($rest:ty),+ $(,)?) => {
		$crate::web_query_schema!($model),
		$crate::web_query_schema!($($rest),+)
	};
}
