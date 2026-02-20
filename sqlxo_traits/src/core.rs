use sqlx::{
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

	type Model: QueryModel + Send + Sync;
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JoinSegment {
	pub descriptor: JoinDescriptor,
	pub kind:       JoinKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JoinPath {
	segments: Vec<JoinSegment>,
}

impl JoinPath {
	pub fn from_join<J: SqlJoin>(join: J, kind: JoinKind) -> Self {
		Self::new(join.descriptor(), kind)
	}

	pub fn new(descriptor: JoinDescriptor, kind: JoinKind) -> Self {
		Self {
			segments: vec![JoinSegment { descriptor, kind }],
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
		&self.segments
	}

	pub fn is_empty(&self) -> bool {
		self.segments.is_empty()
	}

	pub fn first_table(&self) -> Option<&'static str> {
		self.segments.first().map(|seg| seg.descriptor.left_table)
	}
}

pub trait SqlJoin {
	fn descriptor(&self) -> JoinDescriptor;
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

pub trait FullTextSearchable: Sized {
	type FullTextSearchField: Copy + Eq;
	type FullTextSearchConfig: FullTextSearchConfig + Send + Sync;

	fn write_tsvector<W>(
		w: &mut W,
		base_alias: &str,
		config: &Self::FullTextSearchConfig,
	) where
		W: SqlWrite;

	fn write_tsquery<W>(w: &mut W, config: &Self::FullTextSearchConfig)
	where
		W: SqlWrite;

	fn write_rank<W>(
		w: &mut W,
		base_alias: &str,
		config: &Self::FullTextSearchConfig,
	) where
		W: SqlWrite;
}
