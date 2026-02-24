use std::marker::PhantomData;

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
pub trait BuildableInsertQuery<C, Row = <C as QueryContext>::Model>:
	Buildable<C, Row = Row, Plan: Planable<C, Row>>
where
	C: QueryContext,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}

pub struct InsertQueryPlan<
	'a,
	C: QueryContext,
	Row = <C as QueryContext>::Model,
> where
	C::Model: Creatable,
{
	pub(crate) table: &'a str,
	pub(crate) create_model: <C::Model as Creatable>::CreateModel,
	pub(crate) insert_marker_field: Option<&'static str>,
	pub(crate) selection: Option<SelectionList<Row, select::SelectionColumn>>,
	row: PhantomData<Row>,
}

impl<'a, C, Row> InsertQueryPlan<'a, C, Row>
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
impl<'a, C, Row> ExecutablePlan<C> for InsertQueryPlan<'a, C, Row>
where
	C: QueryContext,
	C::Model: Creatable,
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
impl<'a, C, Row> FetchablePlan<C, Row> for InsertQueryPlan<'a, C, Row>
where
	C: QueryContext,
	C::Model: Creatable,
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

impl<'a, C, Row> Planable<C, Row> for InsertQueryPlan<'a, C, Row>
where
	C: QueryContext,
	C::Model: Creatable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}

pub struct InsertQueryBuilder<
	'a,
	C: QueryContext,
	Row = <C as QueryContext>::Model,
> where
	C::Model: Creatable,
{
	pub(crate) table: &'a str,
	pub(crate) create_model: Option<<C::Model as Creatable>::CreateModel>,
	pub(crate) insert_marker_field: Option<&'static str>,
	pub(crate) selection: Option<SelectionList<Row, select::SelectionColumn>>,
	row: PhantomData<Row>,
}

impl<'a, C, Row> InsertQueryBuilder<'a, C, Row>
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

impl<'a, C, Row> Buildable<C> for InsertQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: Creatable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	type Row = Row;
	type Plan = InsertQueryPlan<'a, C, Row>;

	fn from_ctx() -> Self {
		Self {
			table:               C::TABLE,
			create_model:        None,
			insert_marker_field: <C::Model as Creatable>::INSERT_MARKER_FIELD,
			selection:           None,
			row:                 PhantomData,
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
			selection: self.selection,
			row: PhantomData,
		}
	}
}

impl<'a, C, Row> InsertQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: Creatable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	pub fn take<NewRow>(
		self,
		selection: SelectionList<NewRow, select::SelectionEntry>,
	) -> InsertQueryBuilder<'a, C, NewRow>
	where
		NewRow: Send
			+ Sync
			+ Unpin
			+ for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
	{
		InsertQueryBuilder {
			table:               self.table,
			create_model:        self.create_model,
			insert_marker_field: self.insert_marker_field,
			selection:           Some(selection.expect_columns()),
			row:                 PhantomData,
		}
	}
}

impl<'a, C, Row> BuildableInsertQuery<C, Row> for InsertQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: Creatable,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}
