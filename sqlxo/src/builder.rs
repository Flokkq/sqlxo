use sqlx::{
	Executor,
	Postgres,
};
use sqlxo_traits::{
	Filterable,
	QueryContext,
	Sortable,
	SqlWrite,
};

use crate::{
	and,
	blocks::{
		BuildType,
		Expression,
		Page,
		Pagination,
		SelectType,
		SortOrder,
		SqlHead,
		SqlWriter,
	},
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
		let head = SqlHead::new(self.table, build_type.clone());
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

		match build_type {
			BuildType::Select(SelectType::Exists) => {
				w.push_pagination(&Pagination {
					page:      0,
					page_size: 1,
				});
			}
			_ => {
				if let Some(p) = &self.pagination {
					w.push_pagination(p);
				}
			}
		}

		if let BuildType::Select(SelectType::Exists) = build_type {
			w.push(")");
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

	pub async fn fetch_page<'e, E>(
		&self,
		exec: E,
	) -> Result<Page<C::Model>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		#[derive(sqlx::FromRow)]
		struct RowWithCount<M> {
			#[sqlx(flatten)]
			model:       M,
			total_count: i64,
		}

		let rows: Vec<RowWithCount<C::Model>> = self
			.to_query_builder(BuildType::Select(SelectType::StarAndCount))
			.build_query_as::<RowWithCount<C::Model>>()
			.fetch_all(exec)
			.await?;

		let pagination = self.pagination.unwrap_or_default();

		if rows.is_empty() {
			return Ok(Page::new(vec![], pagination, 0));
		}

		let total = rows[0].total_count;
		let items = rows.into_iter().map(|r| r.model).collect();

		Ok(Page::new(items, pagination, total))
	}

	pub async fn exists<'e, E>(&self, exec: E) -> Result<bool, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		#[derive(sqlx::FromRow)]
		struct ExistsRow {
			exists: bool,
		}

		let row: ExistsRow = self
			.to_query_builder(BuildType::Select(SelectType::Exists))
			.build_query_as::<ExistsRow>()
			.fetch_one(exec)
			.await?;

		Ok(row.exists)
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

	#[cfg(any(test, feature = "test-utils"))]
	pub fn sql(&self, build: BuildType) -> String {
		use sqlx::Execute;
		self.to_query_builder(build).build().sql().to_string()
	}
}
