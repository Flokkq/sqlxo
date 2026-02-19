use std::marker::PhantomData;

use sqlx::{
	Executor,
	Postgres,
};
use sqlxo_traits::{
	QueryContext,
	Updatable,
	UpdateModel,
};

use crate::{
	and,
	blocks::{
		BuildableFilter,
		Expression,
		SqlWriter,
		UpdateHead,
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

#[allow(dead_code)]
pub trait BuildableUpdateQuery<C, Row = <C as QueryContext>::Model>:
	Buildable<C, Row = Row, Plan: Planable<C, Row>> + BuildableFilter<C>
where
	C: QueryContext,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}

pub struct UpdateQueryPlan<
	'a,
	C: QueryContext,
	Row = <C as QueryContext>::Model,
> where
	C::Model: Updatable,
{
	pub(crate) where_expr: Option<Expression<C::Query>>,
	pub(crate) table: &'a str,
	pub(crate) update_model: <C::Model as Updatable>::UpdateModel,
	pub(crate) update_marker_field: Option<&'static str>,
	pub(crate) selection: Option<SelectionList<Row>>,
	row: PhantomData<Row>,
}

impl<'a, C, Row> UpdateQueryPlan<'a, C, Row>
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

		let _ = self
			.update_model
			.apply_updates(w.query_builder_mut(), has_marker);

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
impl<'a, C, Row> ExecutablePlan<C> for UpdateQueryPlan<'a, C, Row>
where
	C: QueryContext,
	C::Model: Updatable,
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
impl<'a, C, Row> FetchablePlan<C, Row> for UpdateQueryPlan<'a, C, Row>
where
	C: QueryContext,
	C::Model: Updatable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	async fn fetch_one<'e, E>(&self, exec: E) -> Result<Row, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		let mut qb = self.to_query_builder();
		select::push_returning(&mut qb, self.table, self.selection.as_ref());
		qb.build_query_as::<Row>().fetch_one(exec).await
	}

	async fn fetch_all<'e, E>(&self, exec: E) -> Result<Vec<Row>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		let mut qb = self.to_query_builder();
		select::push_returning(&mut qb, self.table, self.selection.as_ref());
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
		select::push_returning(&mut qb, self.table, self.selection.as_ref());
		qb.build_query_as::<Row>().fetch_optional(exec).await
	}
}

impl<'a, C, Row> Planable<C, Row> for UpdateQueryPlan<'a, C, Row>
where
	C: QueryContext,
	C::Model: Updatable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}

pub struct UpdateQueryBuilder<
	'a,
	C: QueryContext,
	Row = <C as QueryContext>::Model,
> where
	C::Model: Updatable,
{
	pub(crate) table: &'a str,
	pub(crate) where_expr: Option<Expression<C::Query>>,
	pub(crate) update_model: Option<<C::Model as Updatable>::UpdateModel>,
	pub(crate) update_marker_field: Option<&'static str>,
	pub(crate) selection: Option<SelectionList<Row>>,
	row: PhantomData<Row>,
}

impl<'a, C, Row> UpdateQueryBuilder<'a, C, Row>
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

impl<'a, C, Row> Buildable<C> for UpdateQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: Updatable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	type Row = Row;
	type Plan = UpdateQueryPlan<'a, C, Row>;

	fn from_ctx() -> Self {
		Self {
			table:               C::TABLE,
			where_expr:          None,
			update_model:        None,
			update_marker_field: <C::Model as Updatable>::UPDATE_MARKER_FIELD,
			selection:           None,
			row:                 PhantomData,
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
			selection: self.selection,
			row: PhantomData,
		}
	}
}

impl<'a, C, Row> UpdateQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: Updatable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	pub fn take<NewRow>(
		self,
		selection: SelectionList<NewRow>,
	) -> UpdateQueryBuilder<'a, C, NewRow>
	where
		NewRow: Send
			+ Sync
			+ Unpin
			+ for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
	{
		UpdateQueryBuilder {
			table:               self.table,
			where_expr:          self.where_expr,
			update_model:        self.update_model,
			update_marker_field: self.update_marker_field,
			selection:           Some(selection),
			row:                 PhantomData,
		}
	}
}

impl<'a, C, Row> BuildableFilter<C> for UpdateQueryBuilder<'a, C, Row>
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

impl<'a, C, Row> BuildableUpdateQuery<C, Row> for UpdateQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: Updatable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}
