#![feature(trait_alias)]
#![forbid(unsafe_code)]
extern crate self as sqlxo;

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
pub mod web;

mod read;
pub use read::{
	ReadQueryBuilder,
	ReadQueryPlan,
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
	pub fn insert() -> ReadQueryBuilder<'a, C> {
		ReadQueryBuilder::from_ctx()
	}
}

#[async_trait::async_trait]
pub trait ExecutablePlan<C: QueryContext> {
	async fn execute<'e, E>(&self, exec: E) -> Result<u64, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>;
}

#[async_trait::async_trait]
pub trait FetchablePlan<C: QueryContext> {
	async fn fetch_one<'e, E>(&self, exec: E) -> Result<C::Model, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>;

	async fn fetch_all<'e, E>(
		&self,
		exec: E,
	) -> Result<Vec<C::Model>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>;

	async fn fetch_optional<'e, E>(
		&self,
		exec: E,
	) -> Result<Option<C::Model>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>;
}

pub trait Planable<C>: ExecutablePlan<C> + FetchablePlan<C>
where
	C: QueryContext,
{
}

pub trait Buildable<C: QueryContext> {
	type Plan: Planable<C>;

	fn from_ctx() -> Self;
	fn build(self) -> Self::Plan;
}
