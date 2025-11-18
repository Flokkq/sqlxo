use sqlx::{
	Executor,
	Postgres,
};
use sqlxo_traits::{
	Filterable,
	QueryContext,
	Sortable,
};

use crate::expression::Expression;
use crate::head::{
	BuildType,
	SelectType,
	SqlHead,
};
use crate::pagination::Pagination;
use crate::sort::SortOrder;
use crate::writer::SqlWriter;
use crate::{
	and,
	order_by,
};

pub struct QueryBuilder<'a, C: QueryContext> {
	pub(crate) table:      &'a str,
	pub(crate) joins:      Option<Vec<C::Join>>,
	pub(crate) where_expr: Option<Expression<C::Query>>,
	pub(crate) sort_expr:  Option<SortOrder<C::Sort>>,
	pub(crate) pagination: Option<Pagination>,
}

impl<'a, C> QueryBuilder<'a, C>
where
	C: QueryContext,
{
	pub fn from_ctx() -> Self {
		Self {
			table:      C::TABLE,
			joins:      None,
			where_expr: None,
			sort_expr:  None,
			pagination: None,
		}
	}

	pub fn join(mut self, j: C::Join) -> Self {
		match &mut self.joins {
			Some(existing) => existing.push(j),
			None => self.joins = Some(vec![j]),
		};

		self
	}

	pub fn r#where(mut self, e: Expression<C::Query>) -> Self {
		match self.where_expr {
			Some(existing) => self.where_expr = Some(and![existing, e]),
			None => self.where_expr = Some(e),
		};

		self
	}

	pub fn order_by(mut self, s: SortOrder<C::Sort>) -> Self {
		match self.sort_expr {
			Some(existing) => self.sort_expr = Some(order_by![existing, s]),
			None => self.sort_expr = Some(s),
		}

		self
	}

	pub fn paginate(mut self, p: Pagination) -> Self {
		self.pagination = Some(p);
		self
	}

	pub fn build(self) -> QueryPlan<'a, C> {
		QueryPlan {
			table:      self.table,
			joins:      self.joins,
			where_expr: self.where_expr,
			sort_expr:  self.sort_expr,
			pagination: self.pagination,
		}
	}
}

pub struct QueryPlan<'a, C: QueryContext> {
	pub(crate) joins:      Option<Vec<C::Join>>,
	pub(crate) where_expr: Option<Expression<C::Query>>,
	pub(crate) sort_expr:  Option<SortOrder<C::Sort>>,
	pub(crate) pagination: Option<Pagination>,
	pub(crate) table:      &'a str,
}

impl<'a, C> QueryPlan<'a, C>
where
	C: QueryContext,
	C::Query: Filterable<Entity = C::Model>,
	C::Sort: Sortable<Entity = C::Model>,
{
	fn to_query_builder(
		&self,
		build_type: BuildType,
	) -> sqlx::QueryBuilder<'static, Postgres> {
		let head = SqlHead::new(self.table, build_type);
		let mut w = SqlWriter::new(head);

		if let Some(js) = &self.joins {
			w.push_joins(js);
		}

		if let Some(e) = &self.where_expr {
			w.push_where(e);
		}

		if let Some(s) = &self.sort_expr {
			w.push_sort(s);
		}

		if let Some(p) = &self.pagination {
			w.push_pagination(p);
		}

		w.into_builder()
	}

	pub async fn fetch_all<'e, E>(
		&self,
		exec: E,
	) -> Result<Vec<C::Model>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		self.to_query_builder(BuildType::Select(SelectType::Star))
			.build_query_as::<C::Model>()
			.fetch_all(exec)
			.await
	}

	pub async fn fetch_one<'e, E>(
		&self,
		exec: E,
	) -> Result<C::Model, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		self.to_query_builder(BuildType::Select(SelectType::Star))
			.build_query_as::<C::Model>()
			.fetch_one(exec)
			.await
	}

	pub async fn fetch_optional<'e, E>(
		&self,
		exec: E,
	) -> Result<Option<C::Model>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		self.to_query_builder(BuildType::Select(SelectType::Star))
			.build_query_as::<C::Model>()
			.fetch_optional(exec)
			.await
	}

	#[cfg(test)]
	pub fn sql(&self, build: BuildType) -> String {
		use sqlx::Execute;
		self.to_query_builder(build).build().sql().to_string()
	}
}
