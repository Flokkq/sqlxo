use sqlx::{
	Executor,
	Postgres,
};
use sqlxo_traits::QueryContext;

use crate::{
	and,
	blocks::{
		BuildableFilter,
		DeleteHead,
		Expression,
		SqlWriter,
	},
	Buildable,
	ExecutablePlan,
	FetchablePlan,
	Planable,
};

/// TODO: this will be useful once multiple sql dialects will be supported
#[allow(dead_code)]
pub trait BuildableDeleteQuery<C>:
	Buildable<C, Plan: Planable<C>> + BuildableFilter<C>
where
	C: QueryContext,
{
}

pub struct DeleteQueryPlan<'a, C: QueryContext> {
	pub(crate) where_expr:          Option<Expression<C::Query>>,
	pub(crate) table:               &'a str,
	pub(crate) is_soft:             bool,
	pub(crate) delete_marker_field: Option<&'a str>,
}

impl<'a, C> DeleteQueryPlan<'a, C>
where
	C: QueryContext,
{
	fn to_query_builder(&self) -> sqlx::QueryBuilder<'static, Postgres> {
		let head =
			DeleteHead::new(self.table, self.is_soft, self.delete_marker_field);
		let mut w = SqlWriter::new(head);

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
impl<'a, C> ExecutablePlan<C> for DeleteQueryPlan<'a, C>
where
	C: QueryContext,
	C::Model: crate::Deletable,
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
impl<'a, C> FetchablePlan<C> for DeleteQueryPlan<'a, C>
where
	C: QueryContext,
	C::Model: crate::Deletable,
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

impl<'a, C> Planable<C> for DeleteQueryPlan<'a, C>
where
	C: QueryContext,
	C::Model: crate::Deletable,
{
}

pub struct DeleteQueryBuilder<'a, C: QueryContext> {
	pub(crate) table:               &'a str,
	pub(crate) where_expr:          Option<Expression<C::Query>>,
	pub(crate) is_soft:             bool,
	pub(crate) delete_marker_field: Option<&'a str>,
}

impl<'a, C> DeleteQueryBuilder<'a, C>
where
	C: QueryContext,
{
	pub fn new_soft(table: &'a str, delete_marker_field: &'a str) -> Self {
		Self {
			table,
			where_expr: None,
			is_soft: true,
			delete_marker_field: Some(delete_marker_field),
		}
	}

	pub fn new_hard(table: &'a str) -> Self {
		Self {
			table,
			where_expr: None,
			is_soft: false,
			delete_marker_field: None,
		}
	}
}

impl<'a, C> Buildable<C> for DeleteQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: crate::Deletable,
{
	type Plan = DeleteQueryPlan<'a, C>;

	fn from_ctx() -> Self {
		Self {
			table:               C::TABLE,
			where_expr:          None,
			is_soft:             <C::Model as crate::Deletable>::IS_SOFT_DELETE,
			delete_marker_field:
				<C::Model as crate::Deletable>::DELETE_MARKER_FIELD,
		}
	}

	fn build(self) -> Self::Plan {
		DeleteQueryPlan {
			where_expr:          self.where_expr,
			table:               self.table,
			is_soft:             self.is_soft,
			delete_marker_field: self.delete_marker_field,
		}
	}
}

impl<'a, C> BuildableFilter<C> for DeleteQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: crate::Deletable,
{
	fn r#where(mut self, e: Expression<<C as QueryContext>::Query>) -> Self {
		match self.where_expr {
			Some(existing) => self.where_expr = Some(and![existing, e]),
			None => self.where_expr = Some(e),
		};

		self
	}
}

impl<'a, C> BuildableDeleteQuery<C> for DeleteQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: crate::Deletable,
{
}
