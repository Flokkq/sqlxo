use sqlx::{
	Executor,
	Postgres,
};
use sqlxo_traits::{
	Creatable,
	CreateModel,
	QueryContext,
};

use crate::{
	blocks::{
		InsertHead,
		SqlWriter,
	},
	Buildable,
	ExecutablePlan,
	FetchablePlan,
	Planable,
};

#[allow(dead_code)]
pub trait BuildableInsertQuery<C>: Buildable<C, Plan: Planable<C>>
where
	C: QueryContext,
{
}

pub struct InsertQueryPlan<'a, C: QueryContext>
where
	C::Model: Creatable,
{
	pub(crate) table:               &'a str,
	pub(crate) create_model:        <C::Model as Creatable>::CreateModel,
	pub(crate) insert_marker_field: Option<&'static str>,
}

impl<'a, C> InsertQueryPlan<'a, C>
where
	C: QueryContext,
	C::Model: Creatable,
{
	fn to_query_builder(&self) -> sqlx::QueryBuilder<'static, Postgres> {
		let head = InsertHead::new(self.table);
		let mut w = SqlWriter::new(head);

		self.create_model
			.apply_inserts(w.query_builder_mut(), self.insert_marker_field);

		w.into_builder()
	}

	#[cfg(any(test, feature = "test-utils"))]
	pub fn sql(&self) -> String {
		use sqlx::Execute;
		self.to_query_builder().build().sql().to_string()
	}
}

#[async_trait::async_trait]
impl<'a, C> ExecutablePlan<C> for InsertQueryPlan<'a, C>
where
	C: QueryContext,
	C::Model: Creatable,
{
	async fn execute<'e, E>(&self, exec: E) -> Result<u64, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		let rows = self
			.to_query_builder()
			.build()
			.execute(exec)
			.await?
			.rows_affected();

		Ok(rows)
	}
}

#[async_trait::async_trait]
impl<'a, C> FetchablePlan<C> for InsertQueryPlan<'a, C>
where
	C: QueryContext,
	C::Model: Creatable,
{
	async fn fetch_one<'e, E>(&self, exec: E) -> Result<C::Model, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		self.to_query_builder()
			.push(" RETURNING *")
			.build_query_as::<C::Model>()
			.fetch_one(exec)
			.await
	}

	async fn fetch_all<'e, E>(
		&self,
		exec: E,
	) -> Result<Vec<C::Model>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		self.to_query_builder()
			.push(" RETURNING *")
			.build_query_as::<C::Model>()
			.fetch_all(exec)
			.await
	}

	async fn fetch_optional<'e, E>(
		&self,
		exec: E,
	) -> Result<Option<C::Model>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		self.to_query_builder()
			.push(" RETURNING *")
			.build_query_as::<C::Model>()
			.fetch_optional(exec)
			.await
	}
}

impl<'a, C> Planable<C> for InsertQueryPlan<'a, C>
where
	C: QueryContext,
	C::Model: Creatable,
{
}

pub struct InsertQueryBuilder<'a, C: QueryContext>
where
	C::Model: Creatable,
{
	pub(crate) table:               &'a str,
	pub(crate) create_model: Option<<C::Model as Creatable>::CreateModel>,
	pub(crate) insert_marker_field: Option<&'static str>,
}

impl<'a, C> InsertQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: Creatable,
{
	pub fn model(
		mut self,
		model: <C::Model as Creatable>::CreateModel,
	) -> Self {
		self.create_model = Some(model);
		self
	}
}

impl<'a, C> Buildable<C> for InsertQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: Creatable,
{
	type Plan = InsertQueryPlan<'a, C>;

	fn from_ctx() -> Self {
		Self {
			table:               C::TABLE,
			create_model:        None,
			insert_marker_field: <C::Model as Creatable>::INSERT_MARKER_FIELD,
		}
	}

	fn build(self) -> Self::Plan {
		let create_model = self
			.create_model
			.expect("create model must be set with .model()");

		InsertQueryPlan {
			table: self.table,
			create_model,
			insert_marker_field: self.insert_marker_field,
		}
	}
}

impl<'a, C> BuildableInsertQuery<C> for InsertQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: Creatable,
{
}
