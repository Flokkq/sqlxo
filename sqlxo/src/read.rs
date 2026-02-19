use smallvec::SmallVec;
use std::marker::PhantomData;

use sqlx::{
	Executor,
	Postgres,
};
use sqlxo_traits::{
	GetDeleteMarker,
	JoinKind,
	JoinPath,
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
		QualifiedColumn,
		ReadHead,
		SelectType,
		SortOrder,
		SqlWriter,
	},
	order_by,
	select::{
		SelectionColumn,
		SelectionList,
	},
	Buildable,
	ExecutablePlan,
	FetchablePlan,
	Planable,
};

/// TODO: this will be useful once multiple sql dialects will be supported
#[allow(dead_code)]
pub trait BuildableReadQuery<C, Row = <C as QueryContext>::Model>:
	Buildable<C, Row = Row, Plan: Planable<C, Row>>
	+ BuildableFilter<C>
	+ BuildableJoin<C>
	+ BuildableSort<C>
	+ BuildablePage<C>
where
	C: QueryContext,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}

pub struct ReadQueryPlan<'a, C: QueryContext, Row = <C as QueryContext>::Model>
{
	pub(crate) joins: Option<Vec<JoinPath>>,
	pub(crate) where_expr: Option<Expression<C::Query>>,
	pub(crate) sort_expr: Option<SortOrder<C::Sort>>,
	pub(crate) pagination: Option<Pagination>,
	pub(crate) table: &'a str,
	pub(crate) include_deleted: bool,
	pub(crate) delete_marker_field: Option<&'a str>,
	pub(crate) selection: Option<SelectionList<Row>>,
	row: PhantomData<Row>,
}

fn build_alias_lookup(
	joins: Option<&[JoinPath]>,
) -> Vec<(&'static str, String)> {
	let mut aliases = Vec::new();

	if let Some(paths) = joins {
		for path in paths {
			let mut alias_prefix = String::new();
			for segment in path.segments() {
				alias_prefix.push_str(segment.descriptor.alias_segment);
				aliases.push((
					segment.descriptor.right_table,
					alias_prefix.clone(),
				));
			}
		}
	}

	aliases
}

fn resolve_alias_for_table(
	table: &'static str,
	column: &'static str,
	base_table: &str,
	aliases: &[(&'static str, String)],
) -> String {
	if table == base_table {
		return base_table.to_string();
	}

	let mut matches =
		aliases.iter().filter(|(tbl, _)| *tbl == table).peekable();

	let Some((_, alias)) = matches.next() else {
		panic!(
			"`take!` requested column `{table}.{column}` but `{table}` is not \
			 part of the join set"
		);
	};

	if matches.peek().is_some() {
		panic!(
			"`take!` requested column `{table}.{column}` but `{table}` is \
			 joined multiple times; disambiguation is not implemented yet"
		);
	}

	alias.clone()
}

fn resolve_selection_columns(
	selection: &[SelectionColumn],
	base_table: &str,
	joins: Option<&[JoinPath]>,
) -> SmallVec<[QualifiedColumn; 4]> {
	let aliases = build_alias_lookup(joins);
	let mut resolved = SmallVec::new();

	for col in selection {
		let table_alias = resolve_alias_for_table(
			col.table, col.column, base_table, &aliases,
		);
		resolved.push(QualifiedColumn {
			table_alias,
			column: col.column,
		});
	}

	resolved
}

impl<'a, C, Row> ReadQueryPlan<'a, C, Row>
where
	C: QueryContext,
{
	fn to_query_builder(
		&self,
		select_type: SelectType,
	) -> sqlx::QueryBuilder<'static, Postgres> {
		let head = ReadHead::new(
			self.table,
			self.select_type_for(select_type.clone()),
		);
		let mut w = SqlWriter::new(head);

		if let Some(js) = &self.joins {
			w.push_joins(js, self.table);
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

	fn select_type_for(&self, base: SelectType) -> SelectType {
		match base {
			SelectType::Star => self
				.selection
				.as_ref()
				.map(|s| {
					SelectType::Columns(resolve_selection_columns(
						s.columns(),
						self.table,
						self.joins.as_deref(),
					))
				})
				.unwrap_or(SelectType::Star),
			other => other,
		}
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
impl<'a, C, Row> FetchablePlan<C, Row> for ReadQueryPlan<'a, C, Row>
where
	C: QueryContext,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	async fn fetch_one<'e, E>(&self, exec: E) -> Result<Row, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		self.to_query_builder(SelectType::Star)
			.build_query_as::<Row>()
			.fetch_one(exec)
			.await
	}

	async fn fetch_all<'e, E>(&self, exec: E) -> Result<Vec<Row>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		self.to_query_builder(SelectType::Star)
			.build_query_as::<Row>()
			.fetch_all(exec)
			.await
	}

	async fn fetch_optional<'e, E>(
		&self,
		exec: E,
	) -> Result<Option<Row>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		self.to_query_builder(SelectType::Star)
			.build_query_as::<Row>()
			.fetch_optional(exec)
			.await
	}
}

#[async_trait::async_trait]
impl<'a, C, Row> ExecutablePlan<C> for ReadQueryPlan<'a, C, Row>
where
	C: QueryContext,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
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

impl<'a, C, Row> Planable<C, Row> for ReadQueryPlan<'a, C, Row>
where
	C: QueryContext,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}

pub struct ReadQueryBuilder<
	'a,
	C: QueryContext,
	Row = <C as QueryContext>::Model,
> {
	pub(crate) table: &'a str,
	pub(crate) joins: Option<Vec<JoinPath>>,
	pub(crate) where_expr: Option<Expression<C::Query>>,
	pub(crate) sort_expr: Option<SortOrder<C::Sort>>,
	pub(crate) pagination: Option<Pagination>,
	pub(crate) include_deleted: bool,
	pub(crate) delete_marker_field: Option<&'a str>,
	pub(crate) selection: Option<SelectionList<Row>>,
	row: PhantomData<Row>,
}

impl<'a, C, Row> ReadQueryBuilder<'a, C, Row>
where
	C: QueryContext,
{
	pub fn include_deleted(mut self) -> Self {
		self.include_deleted = true;
		self
	}
}

impl<'a, C, Row> Buildable<C> for ReadQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	type Row = Row;
	type Plan = ReadQueryPlan<'a, C, Row>;

	fn from_ctx() -> Self {
		Self {
			table:               C::TABLE,
			joins:               None,
			where_expr:          None,
			sort_expr:           None,
			pagination:          None,
			include_deleted:     false,
			delete_marker_field: C::Model::delete_marker_field(),
			selection:           None,
			row:                 PhantomData,
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
			selection:           self.selection,
			row:                 PhantomData,
		}
	}
}

impl<'a, C, Row> ReadQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	pub fn take<NewRow>(
		self,
		selection: SelectionList<NewRow>,
	) -> ReadQueryBuilder<'a, C, NewRow>
	where
		NewRow: Send
			+ Sync
			+ Unpin
			+ for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
	{
		ReadQueryBuilder {
			table:               self.table,
			joins:               self.joins,
			where_expr:          self.where_expr,
			sort_expr:           self.sort_expr,
			pagination:          self.pagination,
			include_deleted:     self.include_deleted,
			delete_marker_field: self.delete_marker_field,
			selection:           Some(selection),
			row:                 PhantomData,
		}
	}
}

impl<'a, C, Row> BuildableFilter<C> for ReadQueryBuilder<'a, C, Row>
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

impl<'a, C, Row> BuildableJoin<C> for ReadQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker,
{
	fn join(self, join: <C as QueryContext>::Join, kind: JoinKind) -> Self {
		self.join_path(JoinPath::from_join(join, kind))
	}

	fn join_path(mut self, path: JoinPath) -> Self {
		if let Some(expected) = path.first_table() {
			assert_eq!(
				expected, self.table,
				"join path must start at base table `{}` but started at `{}`",
				self.table, expected,
			);
		}

		match &mut self.joins {
			Some(existing) => existing.push(path),
			None => self.joins = Some(vec![path]),
		};

		self
	}
}

impl<'a, C, Row> BuildableSort<C> for ReadQueryBuilder<'a, C, Row>
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

impl<'a, C, Row> BuildablePage<C> for ReadQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker,
{
	fn paginate(mut self, p: Pagination) -> Self {
		self.pagination = Some(p);
		self
	}
}

impl<'a, C, Row> BuildableReadQuery<C, Row> for ReadQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}
