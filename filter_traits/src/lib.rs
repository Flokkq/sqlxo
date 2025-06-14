use sqlx::postgres::PgArguments;
use sqlx::query::QueryAs;
use sqlx::Postgres;

pub trait Filterable {
    type Entity: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>;

    fn table_name() -> &'static str;
    fn filter_clause(&self, idx: usize) -> String;

    fn bind<'q>(
        self,
        q: sqlx::query::QueryAs<'q, sqlx::Postgres, Self::Entity, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::QueryAs<'q, sqlx::Postgres, Self::Entity, sqlx::postgres::PgArguments>;
}
