use sqlx::{
	Executor,
	Postgres,
};
use sqlxo_traits::{
	QueryContext,
	UpdateModel,
	Updatable,
};

use crate::{
	and,
	blocks::{
		BuildableFilter,
		Expression,
		SqlWriter,
		UpdateHead,
	},
	Buildable,
	ExecutablePlan,
	FetchablePlan,
	Planable,
};

#[allow(dead_code)]
pub trait BuildableUpdateQuery<C>:
	Buildable<C, Plan: Planable<C>> + BuildableFilter<C>
where
	C: QueryContext,
{
}

pub struct UpdateQueryPlan<'a, C: QueryContext>
where
	C::Model: Updatable,
{
	pub(crate) where_expr:          Option<Expression<C::Query>>,
	pub(crate) table:               &'a str,
	pub(crate) update_model:        <C::Model as Updatable>::UpdateModel,
	pub(crate) update_marker_field: Option<&'static str>,
}

impl<'a, C> UpdateQueryPlan<'a, C>
where
	C: QueryContext,
	C::Model: Updatable,
{
	fn to_query_builder(&self) -> sqlx::QueryBuilder<'static, Postgres> {
		let head = UpdateHead::new(self.table);
		let mut w = SqlWriter::new(head);

		let has_marker = self.update_marker_field.is_some();

		if let Some(marker_field) = self.update_marker_field {
			let qb = w.query_builder_mut();
			qb.push(marker_field);
			qb.push(" = NOW()");
		}

		let _ = self.update_model.apply_updates(w.query_builder_mut(), has_marker);

		if let Some(e) = &self.where_expr {
			w.push_where(e);
		}

		w.into_builder()
	}

	#[cfg(any(test, feature = "test-utils"))]
	pub fn sql(&self) -> String {
		use sqlx::Execute;
		self.to_query_builder().build().sql().to_string()
	}
}

#[async_trait::async_trait]
impl<'a, C> ExecutablePlan<C> for UpdateQueryPlan<'a, C>
where
	C: QueryContext,
	C::Model: Updatable,
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
impl<'a, C> FetchablePlan<C> for UpdateQueryPlan<'a, C>
where
	C: QueryContext,
	C::Model: Updatable,
{
	async fn fetch_one<'e, E>(&self, exec: E) -> Result<C::Model, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		let mut qb = self.to_query_builder();
		qb.push(" RETURNING *");
		qb.build_query_as::<C::Model>().fetch_one(exec).await
	}

	async fn fetch_all<'e, E>(
		&self,
		exec: E,
	) -> Result<Vec<C::Model>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		let mut qb = self.to_query_builder();
		qb.push(" RETURNING *");
		qb.build_query_as::<C::Model>().fetch_all(exec).await
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

impl<'a, C> Planable<C> for UpdateQueryPlan<'a, C>
where
	C: QueryContext,
	C::Model: Updatable,
{
}

pub struct UpdateQueryBuilder<'a, C: QueryContext>
where
	C::Model: Updatable,
{
	pub(crate) table:               &'a str,
	pub(crate) where_expr:          Option<Expression<C::Query>>,
	pub(crate) update_model: Option<<C::Model as Updatable>::UpdateModel>,
	pub(crate) update_marker_field: Option<&'static str>,
}

impl<'a, C> UpdateQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: Updatable,
{
	pub fn model(
		mut self,
		model: <C::Model as Updatable>::UpdateModel,
	) -> Self {
		self.update_model = Some(model);
		self
	}
}

impl<'a, C> Buildable<C> for UpdateQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: Updatable,
{
	type Plan = UpdateQueryPlan<'a, C>;

	fn from_ctx() -> Self {
		Self {
			table:               C::TABLE,
			where_expr:          None,
			update_model:        None,
			update_marker_field: <C::Model as Updatable>::UPDATE_MARKER_FIELD,
		}
	}

	fn build(self) -> Self::Plan {
		let update_model = self
			.update_model
			.expect("update model must be set with .model()");

		UpdateQueryPlan {
			where_expr: self.where_expr,
			table: self.table,
			update_model,
			update_marker_field: self.update_marker_field,
		}
	}
}

impl<'a, C> BuildableFilter<C> for UpdateQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: Updatable,
{
	fn r#where(mut self, e: Expression<<C as QueryContext>::Query>) -> Self {
		match self.where_expr {
			Some(existing) => self.where_expr = Some(and![existing, e]),
			None => self.where_expr = Some(e),
		};

		self
	}
}

impl<'a, C> BuildableUpdateQuery<C> for UpdateQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: Updatable,
{
}

