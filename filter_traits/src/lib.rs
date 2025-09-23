#![feature(trait_alias)]

pub trait QueryModel =
    Send + Clone + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + 'static;

pub trait QueryQuery = Filterable + Send + Clone + Sync;

pub trait QuerySort = Sortable + Send + Clone + Sync;

pub trait Filterable {
    type Entity: QueryModel;

    fn filter_clause(&self, idx: &mut usize) -> String;

    fn bind<'q>(
        self,
        q: sqlx::query::QueryAs<'q, sqlx::Postgres, Self::Entity, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::QueryAs<'q, sqlx::Postgres, Self::Entity, sqlx::postgres::PgArguments>;
}

pub trait QueryContext {
    const TABLE: &'static str;

    type Model: QueryModel;
    type Query: QueryQuery;
    type Sort: QuerySort;
}

pub trait Sortable {
    type Entity: QueryModel;

    fn sort_clause(&self) -> String;
}

pub trait Model {}
