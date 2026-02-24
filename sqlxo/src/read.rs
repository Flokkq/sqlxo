use smallvec::SmallVec;
use std::marker::PhantomData;

use sqlx::{
	postgres::PgRow,
	Executor,
	FromRow,
	Postgres,
};
use sqlxo_traits::{
	AliasedColumn,
	FullTextSearchConfig,
	FullTextSearchable,
	GetDeleteMarker,
	JoinKind,
	JoinNavigationModel,
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
		SelectProjection,
		SelectType,
		SortOrder,
		SqlWriter,
	},
	order_by,
	select::{
		AggregateFunction,
		AggregateSelection,
		GroupByList,
		HavingList,
		HavingPredicate,
		SelectionColumn,
		SelectionEntry,
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

pub(crate) trait DynFullTextSearchPlan: Send + Sync {
	fn write_condition(
		&self,
		w: &mut SqlWriter,
		base_alias: &str,
		joins: Option<&[JoinPath]>,
	);

	fn write_rank_expr(
		&self,
		w: &mut SqlWriter,
		base_alias: &str,
		joins: Option<&[JoinPath]>,
	);

	fn include_rank(&self) -> bool;
}

struct ModelFullTextSearchPlan<M>
where
	M: FullTextSearchable,
{
	config:  M::FullTextSearchConfig,
	_marker: PhantomData<M>,
}

impl<M> ModelFullTextSearchPlan<M>
where
	M: FullTextSearchable,
{
	fn new(config: M::FullTextSearchConfig) -> Self {
		Self {
			config,
			_marker: PhantomData,
		}
	}
}

impl<M> DynFullTextSearchPlan for ModelFullTextSearchPlan<M>
where
	M: FullTextSearchable + Send + Sync + 'static,
	M::FullTextSearchConfig: Send + Sync,
{
	fn write_condition(
		&self,
		w: &mut SqlWriter,
		base_alias: &str,
		joins: Option<&[JoinPath]>,
	) {
		w.push("(");
		M::write_tsvector(w, base_alias, joins, &self.config);
		w.push(") @@ (");
		M::write_tsquery(w, &self.config);
		w.push(")");
	}

	fn write_rank_expr(
		&self,
		w: &mut SqlWriter,
		base_alias: &str,
		joins: Option<&[JoinPath]>,
	) {
		M::write_rank(w, base_alias, joins, &self.config);
	}

	fn include_rank(&self) -> bool {
		self.config.include_rank()
	}
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
	pub(crate) selection: Option<SelectionList<Row, SelectionEntry>>,
	pub(crate) group_by: Option<Vec<SelectionColumn>>,
	pub(crate) having: Option<Vec<HavingPredicate>>,
	pub(crate) full_text_search: Option<Box<dyn DynFullTextSearchPlan>>,
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
		resolved.push(resolve_selection_column(col, base_table, &aliases));
	}

	resolved
}

fn resolve_selection_column(
	column: &SelectionColumn,
	base_table: &str,
	aliases: &[(&'static str, String)],
) -> QualifiedColumn {
	let table_alias = resolve_alias_for_table(
		column.table,
		column.column,
		base_table,
		aliases,
	);
	QualifiedColumn {
		table_alias,
		column: column.column,
	}
}

fn format_aggregate_expression(
	selection: &AggregateSelection,
	base_table: &str,
	aliases: &[(&'static str, String)],
) -> String {
	match selection.column {
		Some(col) => {
			let qualified = resolve_selection_column(&col, base_table, aliases);
			match selection.function {
				AggregateFunction::CountDistinct => format!(
					r#"COUNT(DISTINCT "{}"."{}")"#,
					qualified.table_alias, qualified.column
				),
				_ => format!(
					r#"{}("{}"."{}")"#,
					selection.function.sql_name(),
					qualified.table_alias,
					qualified.column
				),
			}
		}
		None => format!("{}(*)", selection.function.sql_name()),
	}
}

fn write_having_predicate(
	predicate: &HavingPredicate,
	writer: &mut SqlWriter,
	base_table: &str,
	aliases: &[(&'static str, String)],
) {
	let expr =
		format_aggregate_expression(&predicate.selection, base_table, aliases);
	writer.push(&expr);
	writer.push(" ");
	writer.push(predicate.comparator.as_str());
	writer.push(" ");
	predicate.bind_value(writer);
}

impl<'a, C, Row> ReadQueryPlan<'a, C, Row>
where
	C: QueryContext,
	C::Model: JoinNavigationModel,
{
	fn to_query_builder(
		&self,
		select_type: SelectType,
	) -> sqlx::QueryBuilder<'static, Postgres> {
		let effective_select = self.select_type_for(select_type.clone());
		let head = ReadHead::new(self.table, effective_select);
		let mut w = SqlWriter::new(head);

		if let Some(js) = &self.joins {
			w.push_joins(js, self.table);
		}

		self.push_where_clause(&mut w);
		self.push_group_by_clause(&mut w);
		self.push_having_clause(&mut w);

		if let Some(s) = &self.sort_expr {
			w.push_sort(s);
		} else if !matches!(select_type, SelectType::Exists) {
			if let Some(fts) = &self.full_text_search {
				if fts.include_rank() {
					w.push_order_by_raw(|writer| {
						fts.write_rank_expr(
							writer,
							self.table,
							self.joins.as_deref(),
						);
						writer.push(" DESC");
					});
				}
			}
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
		let resolved = match base {
			SelectType::Star => self
				.selection
				.as_ref()
				.map(|s| self.selection_select_type(s))
				.unwrap_or(SelectType::Star),
			other => other,
		};

		self.apply_join_extras(resolved)
	}

	fn selection_select_type(
		&self,
		selection: &SelectionList<Row, SelectionEntry>,
	) -> SelectType {
		let mut has_columns = false;
		let mut has_aggregates = false;
		for entry in selection.entries() {
			match entry {
				SelectionEntry::Column(_) => has_columns = true,
				SelectionEntry::Aggregate(_) => has_aggregates = true,
			}
		}

		if has_columns && has_aggregates && self.group_by.is_none() {
			panic!(
				"`group_by!` must be provided when selecting columns \
				 alongside aggregates"
			);
		}

		if has_columns && !has_aggregates {
			let mut cols: SmallVec<[SelectionColumn; 4]> =
				SmallVec::with_capacity(selection.entries().len());
			for entry in selection.entries() {
				if let SelectionEntry::Column(col) = entry {
					cols.push(*col);
				}
			}
			return SelectType::Columns(resolve_selection_columns(
				&cols,
				self.table,
				self.joins.as_deref(),
			));
		}

		let projections = self.build_projections(selection);
		SelectType::Projection(projections)
	}

	fn build_projections(
		&self,
		selection: &SelectionList<Row, SelectionEntry>,
	) -> Vec<SelectProjection> {
		let aliases = build_alias_lookup(self.joins.as_deref());
		selection
			.entries()
			.iter()
			.enumerate()
			.map(|(idx, entry)| match entry {
				SelectionEntry::Column(col) => {
					let qualified =
						resolve_selection_column(col, self.table, &aliases);
					SelectProjection {
						expression: format!(
							r#""{}"."{}""#,
							qualified.table_alias, qualified.column
						),
						alias:      None,
					}
				}
				SelectionEntry::Aggregate(agg) => {
					let expr =
						format_aggregate_expression(agg, self.table, &aliases);
					let alias = format!(r#"__sqlxo_sel_{}"#, idx);
					SelectProjection {
						expression: expr,
						alias:      Some(alias),
					}
				}
			})
			.collect()
	}

	fn apply_join_extras(&self, select: SelectType) -> SelectType {
		let extras = self.join_projection_columns();
		if extras.is_empty() {
			return select;
		}

		match select {
			SelectType::Star => SelectType::StarWithExtras(extras),
			SelectType::StarAndCount => SelectType::StarAndCountExtras(extras),
			other => other,
		}
	}

	fn join_projection_columns(&self) -> SmallVec<[AliasedColumn; 4]> {
		if self.selection.is_some() {
			return SmallVec::new();
		}

		C::Model::collect_join_columns(self.joins.as_deref(), "")
	}

	fn push_where_clause(&self, w: &mut SqlWriter) {
		let mut has_clause = false;

		if !self.include_deleted {
			if let Some(delete_field) = self.delete_marker_field {
				let qualified =
					format!(r#""{}"."{}""#, self.table, delete_field);
				w.push_where_raw(|writer| {
					writer.push(&qualified);
					writer.push(" IS NULL");
				});
				has_clause = true;
			}
		}

		if let Some(e) = &self.where_expr {
			let wrap = has_clause;
			w.push_where_raw(|writer| {
				if wrap {
					writer.push("(");
					e.write(writer);
					writer.push(")");
				} else {
					e.write(writer);
				}
			});
			has_clause = true;
		}

		if let Some(fts) = &self.full_text_search {
			let wrap = has_clause;
			w.push_where_raw(|writer| {
				if wrap {
					writer.push("(");
				}
				fts.write_condition(writer, self.table, self.joins.as_deref());
				if wrap {
					writer.push(")");
				}
			});
		}
	}

	fn push_group_by_clause(&self, w: &mut SqlWriter) {
		if let Some(columns) = &self.group_by {
			if columns.is_empty() {
				return;
			}
			let resolved = resolve_selection_columns(
				columns,
				self.table,
				self.joins.as_deref(),
			);
			w.push_group_by_columns(&resolved);
		}
	}

	fn push_having_clause(&self, w: &mut SqlWriter) {
		let Some(predicates) = &self.having else {
			return;
		};
		if predicates.is_empty() {
			return;
		}
		let aliases = build_alias_lookup(self.joins.as_deref());
		let table = self.table;
		w.push_having(|writer| {
			for (idx, predicate) in predicates.iter().enumerate() {
				if idx > 0 {
					writer.push(" AND ");
				}
				write_having_predicate(predicate, writer, table, &aliases);
			}
		});
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

		let rows: Vec<PgRow> = self
			.to_query_builder(SelectType::StarAndCount)
			.build()
			.fetch_all(exec)
			.await?;

		let pagination = self.pagination.unwrap_or_default();

		if rows.is_empty() {
			return Ok(Page::new(vec![], pagination, 0));
		}

		let mut total = 0;
		let mut items = Vec::with_capacity(rows.len());
		let hydrate = self.selection.is_none();

		for row in rows {
			let mut parsed = RowWithCount::<C::Model>::from_row(&row)?;
			if hydrate {
				parsed.model.hydrate_navigations(
					self.joins.as_deref(),
					&row,
					"",
				)?;
			}
			total = parsed.total_count;
			items.push(parsed.model);
		}

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

	fn map_pg_row(&self, row: PgRow) -> Result<Row, sqlx::Error>
	where
		Row: HydrateRow<C>,
	{
		<Row as HydrateRow<C>>::from_pg_row(self, row)
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
		let row = self
			.to_query_builder(SelectType::Star)
			.build()
			.fetch_one(exec)
			.await?;

		self.map_pg_row(row)
	}

	async fn fetch_all<'e, E>(&self, exec: E) -> Result<Vec<Row>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		let rows = self
			.to_query_builder(SelectType::Star)
			.build()
			.fetch_all(exec)
			.await?;

		rows.into_iter().map(|row| self.map_pg_row(row)).collect()
	}

	async fn fetch_optional<'e, E>(
		&self,
		exec: E,
	) -> Result<Option<Row>, sqlx::Error>
	where
		E: Executor<'e, Database = Postgres>,
	{
		let row = self
			.to_query_builder(SelectType::Star)
			.build()
			.fetch_optional(exec)
			.await?;

		match row {
			Some(row) => self.map_pg_row(row).map(Some),
			None => Ok(None),
		}
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

trait HydrateRow<C: QueryContext>: Sized {
	fn from_pg_row(
		plan: &ReadQueryPlan<C, Self>,
		row: PgRow,
	) -> Result<Self, sqlx::Error>;
}

impl<C, Row> HydrateRow<C> for Row
where
	C: QueryContext,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, PgRow>,
{
	default fn from_pg_row(
		_plan: &ReadQueryPlan<C, Row>,
		row: PgRow,
	) -> Result<Self, sqlx::Error> {
		Row::from_row(&row)
	}
}

impl<C> HydrateRow<C> for C::Model
where
	C: QueryContext,
	C::Model: JoinNavigationModel,
{
	fn from_pg_row(
		plan: &ReadQueryPlan<C, Self>,
		row: PgRow,
	) -> Result<Self, sqlx::Error> {
		let mut model = Self::from_row(&row)?;
		if plan.selection.is_none() {
			model.hydrate_navigations(plan.joins.as_deref(), &row, "")?;
		}
		Ok(model)
	}
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
	pub(crate) selection: Option<SelectionList<Row, SelectionEntry>>,
	pub(crate) group_by: Option<Vec<SelectionColumn>>,
	pub(crate) having: Option<Vec<HavingPredicate>>,
	pub(crate) full_text_search: Option<Box<dyn DynFullTextSearchPlan>>,
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

	pub fn search(
		mut self,
		config: <C::Model as FullTextSearchable>::FullTextSearchConfig,
	) -> Self
	where
		C::Model: FullTextSearchable + 'static,
		<C::Model as FullTextSearchable>::FullTextSearchConfig:
			Send + Sync + 'static,
	{
		self.full_text_search =
			Some(Box::new(ModelFullTextSearchPlan::<C::Model>::new(config)));
		self
	}
}

impl<'a, C, Row> Buildable<C> for ReadQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker + JoinNavigationModel,
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
			group_by:            None,
			having:              None,
			full_text_search:    None,
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
			group_by:            self.group_by,
			having:              self.having,
			full_text_search:    self.full_text_search,
			row:                 PhantomData,
		}
	}
}

impl<'a, C, Row> ReadQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker + JoinNavigationModel,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
	pub fn take<NewRow>(
		self,
		selection: SelectionList<NewRow, SelectionEntry>,
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
			group_by:            self.group_by,
			having:              self.having,
			full_text_search:    self.full_text_search,
			row:                 PhantomData,
		}
	}

	pub fn group_by(mut self, group_by: GroupByList) -> Self {
		let cols = group_by.into_columns().into_vec();
		self.group_by = Some(cols);
		self
	}

	pub fn having(mut self, having: HavingList) -> Self {
		self.having = Some(having.into_predicates());
		self
	}
}

impl<'a, C, Row> BuildableFilter<C> for ReadQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker + JoinNavigationModel,
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
	C::Model: crate::GetDeleteMarker + JoinNavigationModel,
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
	C::Model: crate::GetDeleteMarker + JoinNavigationModel,
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
	C::Model: crate::GetDeleteMarker + JoinNavigationModel,
{
	fn paginate(mut self, p: Pagination) -> Self {
		self.pagination = Some(p);
		self
	}
}

impl<'a, C, Row> BuildableReadQuery<C, Row> for ReadQueryBuilder<'a, C, Row>
where
	C: QueryContext,
	C::Model: crate::GetDeleteMarker + JoinNavigationModel,
	Row: Send + Sync + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>,
{
}
