use sqlx::{
	Executor,
	Postgres,
};
use sqlxo_traits::{
	GetDeleteMarker,
	QueryContext,
	SqlWrite,
};

use crate::{
	and,
	blocks::{
		BuildableFilter,
		BuildableJoin,
		BuildablePage,
		BuildableSort,
		Expression,
		Page,
		Pagination,
		ReadHead,
		SelectType,
		SortOrder,
		SqlWriter,
	},
	order_by,
	Buildable,
	ExecutablePlan,
	FetchablePlan,
	Planable,
};

/// TODO: this will be useful once multiple sql dialects will be supported
#[allow(dead_code)]
pub trait BuildableReadQuery<C>:
	Buildable<C, Plan: Planable<C>>
	+ BuildableFilter<C>
	+ BuildableJoin<C>
	+ BuildableSort<C>
	+ BuildablePage<C>
where
	C: QueryContext,
{
}

pub struct ReadQueryPlan<'a, C: QueryContext> {
	pub(crate) joins:               Option<Vec<C::Join>>,
	pub(crate) where_expr:          Option<Expression<C::Query>>,
	pub(crate) sort_expr:           Option<SortOrder<C::Sort>>,
	pub(crate) pagination:          Option<Pagination>,
	pub(crate) table:               &'a str,
	pub(crate) include_deleted:     bool,
	pub(crate) delete_marker_field: Option<&'a str>,
}

impl<'a, C> ReadQueryPlan<'a, C>
where
	C: QueryContext,
{
	fn to_query_builder(
		&self,
		select_type: SelectType,
	) -> sqlx::QueryBuilder<'static, Postgres> {
		let head = ReadHead::new(self.table, select_type.clone());
		let mut w = SqlWriter::new(head);

		if let Some(js) = &self.joins {
			w.push_joins(js);
		}

		self.push_where_clause(&mut w);

		if let Some(s) = &self.sort_expr {
			w.push_sort(s);
		}

		if let SelectType::Exists = select_type {
			w.push_pagination(&Pagination {
				page:      0,
				page_size: 1,
			});
		} else if let Some(p) = &self.pagination {
			w.push_pagination(p);
		}

		if let SelectType::Exists = select_type {
			w.push(")");
		}

		w.into_builder()
	}

	fn push_where_clause(&self, w: &mut SqlWriter) {
		if self.include_deleted {
			if let Some(e) = &self.where_expr {
				w.push_where(e);
			}
			return;
		}

		let Some(delete_field) = self.delete_marker_field else {
			if let Some(e) = &self.where_expr {
				w.push_where(e);
			}
			return;
		};

		w.push_soft_delete_filter(delete_field, self.where_expr.as_ref());
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
			.to_query_builder(SelectType::StarAndCount)
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
			.to_query_builder(SelectType::Exists)
			.build_query_as::<ExistsRow>()
			.fetch_one(exec)
			.await?;

		Ok(row.exists)
	}

	#[cfg(any(test, feature = "test-utils"))]
	pub fn sql(&self, build: SelectType) -> String {
		use sqlx::Execute;
		self.to_query_builder(build).build().sql().to_string()
	}
}

#[async_trait::async_trait]
impl<'a, C> FetchablePlan<C> for ReadQueryPlan<'a, C>
where
	C: QueryContext,
{
	async fn fetch_one<'e, E>(&self, exec: E) -> Result<C::Model, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		self.to_query_builder(SelectType::Star)
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
		self.to_query_builder(SelectType::Star)
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
		self.to_query_builder(SelectType::Star)
			.build_query_as::<C::Model>()
			.fetch_optional(exec)
			.await
	}
}

#[async_trait::async_trait]
impl<'a, C> ExecutablePlan<C> for ReadQueryPlan<'a, C>
where
	C: QueryContext,
{
	async fn execute<'e, E>(&self, exec: E) -> Result<u64, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		let rows = self
			.to_query_builder(SelectType::Star)
			.build()
			.execute(exec)
			.await?
			.rows_affected();

		Ok(rows)
	}
}

impl<'a, C> Planable<C> for ReadQueryPlan<'a, C> where C: QueryContext {}

pub struct ReadQueryBuilder<'a, C: QueryContext> {
	pub(crate) table:               &'a str,
	pub(crate) joins:               Option<Vec<C::Join>>,
	pub(crate) where_expr:          Option<Expression<C::Query>>,
	pub(crate) sort_expr:           Option<SortOrder<C::Sort>>,
	pub(crate) pagination:          Option<Pagination>,
	pub(crate) include_deleted:     bool,
	pub(crate) delete_marker_field: Option<&'a str>,
}

impl<'a, C> ReadQueryBuilder<'a, C>
where
	C: QueryContext,
{
	pub fn include_deleted(mut self) -> Self {
		self.include_deleted = true;
		self
	}
}

impl<'a, C> Buildable<C> for ReadQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker,
{
	type Plan = ReadQueryPlan<'a, C>;

	fn from_ctx() -> Self {
		Self {
			table:               C::TABLE,
			joins:               None,
			where_expr:          None,
			sort_expr:           None,
			pagination:          None,
			include_deleted:     false,
			delete_marker_field: C::Model::delete_marker_field(),
		}
	}

	fn build(self) -> Self::Plan {
		ReadQueryPlan {
			joins:               self.joins,
			where_expr:          self.where_expr,
			sort_expr:           self.sort_expr,
			pagination:          self.pagination,
			table:               self.table,
			include_deleted:     self.include_deleted,
			delete_marker_field: self.delete_marker_field,
		}
	}
}

impl<'a, C> BuildableFilter<C> for ReadQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker,
{
	fn r#where(mut self, e: Expression<<C as QueryContext>::Query>) -> Self {
		match self.where_expr {
			Some(existing) => self.where_expr = Some(and![existing, e]),
			None => self.where_expr = Some(e),
		};

		self
	}
}

impl<'a, C> BuildableJoin<C> for ReadQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker,
{
	fn join(mut self, j: <C as QueryContext>::Join) -> Self {
		match &mut self.joins {
			Some(existing) => existing.push(j),
			None => self.joins = Some(vec![j]),
		};

		self
	}
}

impl<'a, C> BuildableSort<C> for ReadQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker,
{
	fn order_by(mut self, s: SortOrder<<C as QueryContext>::Sort>) -> Self {
		match self.sort_expr {
			Some(existing) => self.sort_expr = Some(order_by![existing, s]),
			None => self.sort_expr = Some(s),
		}

		self
	}
}

impl<'a, C> BuildablePage<C> for ReadQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker,
{
	fn paginate(mut self, p: Pagination) -> Self {
		self.pagination = Some(p);
		self
	}
}

impl<'a, C> BuildableReadQuery<C> for ReadQueryBuilder<'a, C>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker,
{
}
