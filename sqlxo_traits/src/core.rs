use smallvec::SmallVec;
use sqlx::{
	postgres::PgRow,
	prelude::Type,
	Postgres,
};

pub trait QueryModel =
	Send + Clone + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>;

pub trait FilterQuery = Filterable + Clone;

pub trait QuerySort = Sortable + Clone;

pub trait Filterable {
	type Entity: QueryModel;

	fn write<W: SqlWrite>(&self, w: &mut W);
}

pub trait SqlWrite {
	fn push(&mut self, s: &str);

	fn bind<T>(&mut self, value: T)
	where
		T: sqlx::Encode<'static, Postgres> + Send + 'static,
		T: Type<Postgres>;
}

pub trait QueryContext: Send + Sync + 'static {
	const TABLE: &'static str;

	type Model: QueryModel
		+ Send
		+ Sync
		+ JoinNavigationModel
		+ WebJoinGraph
		+ PrimaryKey;
	type Query: FilterQuery + Send + Sync;
	type Sort: QuerySort + Send + Sync;
	type Join: SqlJoin + Send + Sync;
}

pub trait Sortable {
	type Entity: QueryModel;

	fn sort_clause(&self) -> String;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinKind {
	Left,
	Inner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinDescriptor {
	pub left_table:    &'static str,
	pub left_field:    &'static str,
	pub right_table:   &'static str,
	pub right_field:   &'static str,
	pub alias_segment: &'static str,
	pub identifier:    &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinSegment {
	pub descriptor: JoinDescriptor,
	pub kind:       JoinKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinPath {
	segments: Vec<JoinSegment>,
	start:    usize,
}

impl Default for JoinPath {
	fn default() -> Self {
		Self {
			segments: Vec::new(),
			start:    0,
		}
	}
}

impl JoinPath {
	pub fn from_join<J: SqlJoin>(join: J, kind: JoinKind) -> Self {
		Self::new(join.descriptor(), kind)
	}

	pub fn new(descriptor: JoinDescriptor, kind: JoinKind) -> Self {
		Self {
			segments: vec![JoinSegment { descriptor, kind }],
			start:    0,
		}
	}

	pub fn then<J: SqlJoin>(mut self, join: J, kind: JoinKind) -> Self {
		let descriptor = join.descriptor();

		if let Some(prev) = self.segments.last() {
			assert_eq!(
				prev.descriptor.right_table, descriptor.left_table,
				"Invalid join path: expected next hop to start at `{}` but \
				 found `{}`",
				prev.descriptor.right_table, descriptor.left_table,
			);
		}

		self.segments.push(JoinSegment { descriptor, kind });
		self
	}

	pub fn segments(&self) -> &[JoinSegment] {
		&self.segments[self.start..]
	}

	pub fn len(&self) -> usize {
		self.segments().len()
	}

	pub fn append(&mut self, tail: &JoinPath) {
		if tail.is_empty() {
			return;
		}

		let Some(last) = self.segments().last() else {
			return;
		};
		let Some(first) = tail.segments().first() else {
			return;
		};

		assert_eq!(
			last.descriptor.right_table, first.descriptor.left_table,
			"Invalid join append: left table `{}` does not match `{}`",
			last.descriptor.right_table, first.descriptor.left_table,
		);

		self.segments.extend_from_slice(tail.segments());
	}

	pub fn strip_prefix(&self, len: usize) -> Option<Self> {
		let new_start = self.start + len;
		if new_start > self.segments.len() {
			return None;
		}

		Some(Self {
			segments: self.segments.clone(),
			start:    new_start,
		})
	}

	pub fn tail(&self) -> Option<Self> {
		if self.segments.len() - self.start <= 1 {
			None
		} else {
			Some(Self {
				segments: self.segments.clone(),
				start:    self.start + 1,
			})
		}
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}

	pub fn first_table(&self) -> Option<&'static str> {
		self.segments().first().map(|seg| seg.descriptor.left_table)
	}

	pub fn alias(&self) -> String {
		self.alias_prefix(self.len())
	}

	pub fn alias_prefix(&self, len: usize) -> String {
		assert!(len <= self.len());
		let mut alias = String::new();
		let end = self.start + len;
		for segment in &self.segments[..end] {
			alias.push_str(segment.descriptor.alias_segment);
		}
		alias
	}
}

pub trait SqlJoin {
	fn descriptor(&self) -> JoinDescriptor;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AliasedColumn {
	pub table_alias: String,
	pub column:      &'static str,
	pub alias:       String,
}

impl AliasedColumn {
	pub fn new(
		table_alias: impl Into<String>,
		column: &'static str,
		alias: impl Into<String>,
	) -> Self {
		Self {
			table_alias: table_alias.into(),
			column,
			alias: alias.into(),
		}
	}
}

#[derive(PartialEq, Eq)]
pub enum JoinValue<T> {
	NotLoaded,
	Missing,
	Loaded(T),
}

impl<T> Default for JoinValue<T> {
	fn default() -> Self {
		Self::NotLoaded
	}
}

impl<T: Clone> Clone for JoinValue<T> {
	fn clone(&self) -> Self {
		match self {
			Self::NotLoaded => Self::NotLoaded,
			Self::Missing => Self::Missing,
			Self::Loaded(v) => Self::Loaded(v.clone()),
		}
	}
}

impl<T: std::fmt::Debug> std::fmt::Debug for JoinValue<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::NotLoaded => f.write_str("JoinValue::NotLoaded"),
			Self::Missing => f.write_str("JoinValue::Missing"),
			Self::Loaded(v) => {
				f.debug_tuple("JoinValue::Loaded").field(v).finish()
			}
		}
	}
}

pub trait JoinLoadable: Sized {
	fn project_join_columns(
		alias: &str,
		out: &mut SmallVec<[AliasedColumn; 4]>,
	);

	fn hydrate_from_join(
		row: &PgRow,
		alias: &str,
	) -> Result<Option<Self>, sqlx::Error>;
}

pub trait JoinNavigationModel {
	fn collect_join_columns(
		joins: Option<&[JoinPath]>,
		base_alias: &str,
	) -> SmallVec<[AliasedColumn; 4]>;

	fn hydrate_navigations(
		&mut self,
		joins: Option<&[JoinPath]>,
		row: &PgRow,
		base_alias: &str,
	) -> Result<(), sqlx::Error>;
}

pub trait WebJoinGraph {
	fn resolve_join_path(segments: &[&str], kind: JoinKind)
		-> Option<JoinPath>;
}

pub trait PrimaryKey {
	const PRIMARY_KEY: &'static [&'static str];
}

pub trait Model {}

pub trait Deletable {
	const IS_SOFT_DELETE: bool;
	const DELETE_MARKER_FIELD: Option<&'static str>;
}

pub trait GetDeleteMarker {
	fn delete_marker_field() -> Option<&'static str>;
}

impl<T> GetDeleteMarker for T {
	default fn delete_marker_field() -> Option<&'static str> {
		None
	}
}

impl<T: Deletable> GetDeleteMarker for T {
	fn delete_marker_field() -> Option<&'static str> {
		T::DELETE_MARKER_FIELD
	}
}

pub trait Updatable {
	type UpdateModel: UpdateModel<Entity = Self>;
	const UPDATE_MARKER_FIELD: Option<&'static str>;
}

pub trait UpdateModel: Clone + Send + Sync {
	type Entity: QueryModel;

	fn apply_updates(
		&self,
		qb: &mut sqlx::QueryBuilder<'static, sqlx::Postgres>,
		has_previous: bool,
	) -> Vec<&'static str>;
}

pub trait Creatable {
	type CreateModel: CreateModel<Entity = Self>;
	const INSERT_MARKER_FIELD: Option<&'static str>;
}

pub trait CreateModel: Clone + Send + Sync {
	type Entity: QueryModel;

	fn apply_inserts(
		&self,
		qb: &mut sqlx::QueryBuilder<'static, sqlx::Postgres>,
		insert_marker_field: Option<&'static str>,
	);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchWeight {
	#[default]
	A,
	B,
	C,
	D,
}

impl SearchWeight {
	pub fn to_char(self) -> char {
		match self {
			Self::A => 'A',
			Self::B => 'B',
			Self::C => 'C',
			Self::D => 'D',
		}
	}

	pub fn sql_literal(self) -> &'static str {
		match self {
			Self::A => "'A'",
			Self::B => "'B'",
			Self::C => "'C'",
			Self::D => "'D'",
		}
	}
}

pub trait FullTextSearchConfig {
	fn include_rank(&self) -> bool;
}

pub trait FullTextSearchConfigBuilder: FullTextSearchConfig {
	fn new_with_query(query: String) -> Self;
	fn apply_language(self, language: Option<String>) -> Self;
	fn apply_rank(self, include_rank: Option<bool>) -> Self;
}

pub trait FullTextSearchable: Sized {
	type FullTextSearchField: Copy + Eq;
	type FullTextSearchConfig: FullTextSearchConfig + Send + Sync;
	type FullTextSearchJoin: Copy + Eq;

	fn write_tsvector<W>(
		w: &mut W,
		base_alias: &str,
		joins: Option<&[JoinPath]>,
		config: &Self::FullTextSearchConfig,
	) where
		W: SqlWrite;

	fn write_tsquery<W>(w: &mut W, config: &Self::FullTextSearchConfig)
	where
		W: SqlWrite;

	fn write_rank<W>(
		w: &mut W,
		base_alias: &str,
		joins: Option<&[JoinPath]>,
		config: &Self::FullTextSearchConfig,
	) where
		W: SqlWrite;
}
