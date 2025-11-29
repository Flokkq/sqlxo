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

pub trait SqlJoin {
	fn to_sql(&self) -> String;

	fn kind(&self) -> JoinKind;
}

pub trait Model {}
