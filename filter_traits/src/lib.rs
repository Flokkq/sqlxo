pub trait Filterable {
    type Entity: Send + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow>;

    fn filter_clause(&self, idx: &mut usize) -> String;

    fn bind<'q>(
        self,
        q: sqlx::query::QueryAs<'q, sqlx::Postgres, Self::Entity, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::QueryAs<'q, sqlx::Postgres, Self::Entity, sqlx::postgres::PgArguments>;
}

pub trait QueryContext {
    const TABLE: &'static str;

    type Model: Send + Clone + Unpin + for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + 'static;
    type Query: Filterable + Send + Clone + Sync;
    type Sort: Sortable + Send + Clone + Sync;
}

pub trait Sortable {}

pub trait Model {}
