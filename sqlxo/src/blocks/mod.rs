use sqlx::{
	Postgres,
	Type,
};
use sqlxo_traits::{
	Filterable,
	JoinKind,
	JoinPath,
	Sortable,
};
use sqlxo_traits::{
	QueryContext,
	SqlWrite,
};

mod expression;
mod head;
mod pagination;
mod sort;

pub use expression::Expression;
pub use head::{
	AggregationType,
	DeleteHead,
	InsertHead,
	QualifiedColumn,
	ReadHead,
	SelectType,
	UpdateHead,
};
pub use pagination::{
	Page,
	Pagination,
};
pub use sort::SortOrder;

use crate::blocks::head::ToHead;

/// TODO: add modifier traits
/// and()
/// or()
/// allow to wrap existing r#where clause in a and/or statement with another
pub trait BuildableFilter<C: QueryContext> {
	fn r#where(self, e: Expression<C::Query>) -> Self;
}

pub trait BuildableJoin<C: QueryContext> {
	fn join(self, join: C::Join, kind: JoinKind) -> Self;

	fn join_path(self, path: JoinPath) -> Self;
}

pub trait BuildableSort<C: QueryContext> {
	fn order_by(self, s: SortOrder<C::Sort>) -> Self;
}

pub trait BuildablePage<C: QueryContext> {
	fn paginate(self, p: Pagination) -> Self;
}

pub struct SqlWriter {
	qb:             sqlx::QueryBuilder<'static, Postgres>,
	has_join:       bool,
	has_where:      bool,
	has_sort:       bool,
	has_pagination: bool,
}

impl SqlWriter {
	pub fn new(head: impl ToHead) -> Self {
		let qb =
			sqlx::QueryBuilder::<Postgres>::new(head.to_head().to_string());

		Self {
			qb,
			has_join: false,
			has_where: false,
			has_sort: false,
			has_pagination: false,
		}
	}

	pub fn into_builder(self) -> sqlx::QueryBuilder<'static, Postgres> {
		self.qb
	}

	/// Get mutable access to the underlying QueryBuilder for advanced
	/// operations
	pub fn query_builder_mut(
		&mut self,
	) -> &mut sqlx::QueryBuilder<'static, Postgres> {
		&mut self.qb
	}

	pub fn push_joins(&mut self, joins: &[JoinPath], base_table: &str) {
		if self.has_join || joins.is_empty() {
			return;
		}

		for path in joins {
			self.push_join_path(path, base_table);
		}

		self.has_join = true;
	}

	fn push_join_path(&mut self, path: &JoinPath, base_table: &str) {
		if path.is_empty() {
			return;
		}

		let mut left_alias = base_table.to_string();
		let mut alias_prefix = String::new();

		for segment in path.segments() {
			alias_prefix.push_str(segment.descriptor.alias_segment);
			let right_alias = alias_prefix.clone();
			let join_word = match segment.kind {
				JoinKind::Inner => " INNER JOIN ",
				JoinKind::Left => " LEFT JOIN ",
			};

			let clause = format!(
				r#"{join}{table} AS "{alias}" ON "{left}"."{left_field}" = "{alias}"."{right_field}""#,
				join = join_word,
				table = segment.descriptor.right_table,
				alias = &right_alias,
				left = &left_alias,
				left_field = segment.descriptor.left_field,
				right_field = segment.descriptor.right_field,
			);

			self.qb.push(clause);
			left_alias = right_alias;
		}
	}

	pub fn push_where<F: Filterable>(&mut self, expr: &Expression<F>) {
		if self.has_where {
			return;
		}

		self.qb.push(" WHERE ");
		self.has_where = true;
		expr.write(self);
	}

	pub fn push_soft_delete_filter<F: Filterable>(
		&mut self,
		delete_field: &str,
		existing_expr: Option<&Expression<F>>,
	) {
		if self.has_where {
			return;
		}

		self.qb.push(" WHERE ");
		self.has_where = true;

		// Add soft delete filter
		self.qb.push(delete_field);
		self.qb.push(" IS NULL");

		// If there's an existing expression, AND it
		if let Some(expr) = existing_expr {
			self.qb.push(" AND (");
			expr.write(self);
			self.qb.push(")");
		}
	}

	pub fn push_sort<S: Sortable>(&mut self, sort: &SortOrder<S>) {
		if self.has_sort {
			return;
		}

		self.qb.push(" ORDER BY ");
		self.has_sort = true;
		self.qb.push(sort.to_sql());
	}

	pub fn push_pagination(&mut self, p: &Pagination) {
		if self.has_pagination {
			return;
		}

		self.qb.push(" LIMIT ");
		self.bind(p.page_size);
		self.qb.push(" OFFSET ");
		self.bind(p.page * p.page_size);
	}
}

impl SqlWrite for SqlWriter {
	fn push(&mut self, s: &str) {
		self.qb.push(s);
	}

	fn bind<T>(&mut self, value: T)
	where
		T: sqlx::Encode<'static, Postgres> + Send + 'static,
		T: Type<Postgres>,
	{
		self.qb.push_bind(value);
	}
}
