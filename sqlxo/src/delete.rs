use std::marker::PhantomData;

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
	select::{
		self,
		SelectionList,
	},
	Buildable,
	ExecutablePlan,
	FetchablePlan,
	Planable,
};

/// TODO: this will be useful once multiple sql dialects will be supported
#[allow(dead_code)]
pub trait BuildableDeleteQuery<C, Row = <C as QueryContext>::Model>:
	Buildable<C, Row = Row, Plan: Planable<C, Row>> + BuildableFilter<C>
where
	C: QueryContext,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}

pub struct DeleteQueryPlan<
	'a,
	C: QueryContext,
	Row = <C as QueryContext>::Model,
> {
	pub(crate) where_expr: Option<Expression<C::Query>>,
	pub(crate) table: &'a str,
	pub(crate) is_soft: bool,
	pub(crate) delete_marker_field: Option<&'a str>,
	pub(crate) selection: Option<SelectionList<C::Model, Row>>,
	row: PhantomData<Row>,
}

impl<'a, C, Row> DeleteQueryPlan<'a, C, Row>
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

	fn push_returning(&self, qb: &mut sqlx::QueryBuilder<'static, Postgres>) {
		select::push_returning(qb, self.table, self.selection.as_ref());
	}

	#[cfg(any(test, feature = "test-utils"))]
	pub fn sql(&self) -> String {
		use sqlx::Execute;
		self.to_query_builder().build().sql().to_string()
	}
}

#[async_trait::async_trait]
impl<'a, C, Row> ExecutablePlan<C> for DeleteQueryPlan<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::Deletable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
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
impl<'a, C, Row> FetchablePlan<C, Row> for DeleteQueryPlan<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::Deletable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	async fn fetch_one<'e, E>(&self, exec: E) -> Result<Row, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		let mut qb = self.to_query_builder();
		self.push_returning(&mut qb);
		qb.build_query_as::<Row>().fetch_one(exec).await
	}

	async fn fetch_all<'e, E>(&self, exec: E) -> Result<Vec<Row>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		let mut qb = self.to_query_builder();
		self.push_returning(&mut qb);
		qb.build_query_as::<Row>().fetch_all(exec).await
	}

	async fn fetch_optional<'e, E>(
		&self,
		exec: E,
	) -> Result<Option<Row>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		let mut qb = self.to_query_builder();
		self.push_returning(&mut qb);
		qb.build_query_as::<Row>().fetch_optional(exec).await
	}
}

impl<'a, C, Row> Planable<C, Row> for DeleteQueryPlan<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::Deletable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}

pub struct DeleteQueryBuilder<
	'a,
	C: QueryContext,
	Row = <C as QueryContext>::Model,
> {
	pub(crate) table: &'a str,
	pub(crate) where_expr: Option<Expression<C::Query>>,
	pub(crate) is_soft: bool,
	pub(crate) delete_marker_field: Option<&'a str>,
	pub(crate) selection: Option<SelectionList<C::Model, Row>>,
	row: PhantomData<Row>,
}

impl<'a, C, Row> DeleteQueryBuilder<'a, C, Row>
where
	C: QueryContext,
{
	pub fn new_soft(table: &'a str, delete_marker_field: &'a str) -> Self {
		Self {
			table,
			where_expr: None,
			is_soft: true,
			delete_marker_field: Some(delete_marker_field),
			selection: None,
			row: PhantomData,
		}
	}

	pub fn new_hard(table: &'a str) -> Self {
		Self {
			table,
			where_expr: None,
			is_soft: false,
			delete_marker_field: None,
			selection: None,
			row: PhantomData,
		}
	}
}

impl<'a, C, Row> Buildable<C> for DeleteQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::Deletable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	type Row = Row;
	type Plan = DeleteQueryPlan<'a, C, Row>;

	fn from_ctx() -> Self {
		Self {
			table:               C::TABLE,
			where_expr:          None,
			is_soft:             <C::Model as crate::Deletable>::IS_SOFT_DELETE,
			delete_marker_field:
				<C::Model as crate::Deletable>::DELETE_MARKER_FIELD,
			selection:           None,
			row:                 PhantomData,
		}
	}

	fn build(self) -> Self::Plan {
		DeleteQueryPlan {
			where_expr:          self.where_expr,
			table:               self.table,
			is_soft:             self.is_soft,
			delete_marker_field: self.delete_marker_field,
			selection:           self.selection,
			row:                 PhantomData,
		}
	}
}

impl<'a, C, Row> DeleteQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::Deletable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	pub fn take<NewRow>(
		self,
		selection: SelectionList<C::Model, NewRow>,
	) -> DeleteQueryBuilder<'a, C, NewRow>
	where
		NewRow: Send
			+ Sync
			+ Unpin
			+ for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
	{
		DeleteQueryBuilder {
			table:               self.table,
			where_expr:          self.where_expr,
			is_soft:             self.is_soft,
			delete_marker_field: self.delete_marker_field,
			selection:           Some(selection),
			row:                 PhantomData,
		}
	}
}

impl<'a, C, Row> BuildableFilter<C> for DeleteQueryBuilder<'a, C, Row>
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

impl<'a, C, Row> BuildableDeleteQuery<C, Row> for DeleteQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::Deletable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}
