use core::fmt;
use std::fmt::{Display, Formatter};

use filter_traits::{Filterable, QueryContext, Sortable, SqlJoin, SqlWrite};
use sqlx::{Postgres, Type};

pub mod repo;

pub struct SqlWriter {
    qb: sqlx::QueryBuilder<'static, Postgres>,
    has_join: bool,
    has_where: bool,
    has_sort: bool,
    has_pagination: bool,
}

impl SqlWriter {
    pub fn new(head: SqlHead) -> Self {
        let qb = sqlx::QueryBuilder::<Postgres>::new(head.to_string());

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

    pub fn push_joins<J: SqlJoin>(&mut self, joins: &Vec<J>) {
        if self.has_join {
            return;
        }

        for j in joins {
            self.qb.push(j.to_sql());
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

    pub fn push_sort<S: Sortable>(&mut self, sort: &SortOrder<S>) {
        if self.has_sort {
            return;
        }

        self.qb.push(" ORDER BY ");
        self.has_sort = true;
        self.qb.push(sort.to_sql());
    }

    fn push_pagination(&mut self, p: &Pagination) {
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

#[derive(PartialEq, Debug, Clone)]
pub enum Expression<T: Filterable> {
    And(Vec<Expression<T>>),
    Or(Vec<Expression<T>>),
    Leaf(T),
}

// Wrap raw leaf
impl<T> From<T> for Expression<T>
where
    T: Filterable,
{
    fn from(t: T) -> Self {
        Expression::Leaf(t)
    }
}

impl<T: Filterable> Expression<T> {
    pub fn write(&self, w: &mut SqlWriter) {
        match self {
            Expression::Leaf(q) => q.write(w),
            Expression::And(xs) => {
                w.push("(");
                for (i, x) in xs.iter().enumerate() {
                    if i > 0 {
                        w.push(" AND ");
                    }

                    x.write(w);
                }
                w.push(")");
            }
            Expression::Or(xs) => {
                w.push("(");
                for (i, x) in xs.iter().enumerate() {
                    if i > 0 {
                        w.push(" OR ");
                    }

                    x.write(w);
                }
                w.push(")");
            }
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct SortOrder<T: Sortable>(Vec<T>);

impl<T> SortOrder<T>
where
    T: Sortable,
{
    pub fn to_sql(&self) -> String {
        let mut out = String::new();

        for (i, s) in self.0.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }

            out.push_str(&s.sort_clause());
        }

        out
    }
}

impl<T: Sortable> From<Vec<T>> for SortOrder<T> {
    fn from(v: Vec<T>) -> Self {
        Self(v)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Pagination {
    pub page: i64,

    pub page_size: i64,
}

impl Pagination {
    pub fn all() -> Self {
        Self {
            page: 0,
            page_size: i32::MAX as i64,
        }
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Pagination::all()
    }
}

pub enum SelectType {
    Star,
    Aggregation(AggregationType),
}

pub enum AggregationType {
    Max,
    Min,
    Count,
    Avg,
}

pub enum BuildType {
    Select(SelectType),
    Update,
    Delete,
    #[cfg(test)]
    Raw,
}

pub struct SqlHead<'a> {
    build: BuildType,
    table: &'a str,
}

impl<'a> SqlHead<'a> {
    pub fn new(table: &'a str, build: BuildType) -> Self {
        Self { build, table }
    }
}

impl Display for AggregationType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            AggregationType::Max => f.write_str("MAX"),
            AggregationType::Min => f.write_str("MIN"),
            AggregationType::Count => f.write_str("COUNT"),
            AggregationType::Avg => f.write_str("AVG"),
        }
    }
}

impl<'a> Display for SqlHead<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.build {
            BuildType::Select(SelectType::Star) => {
                write!(f, "SELECT * FROM {}", self.table)
            }
            BuildType::Select(SelectType::Aggregation(agg)) => {
                write!(f, "SELECT {}(*) FROM {}", agg, self.table)
            }
            BuildType::Update => write!(f, "UPDATE {}", self.table),
            BuildType::Delete => write!(f, "DELETE FROM {}", self.table),
            #[cfg(test)]
            BuildType::Raw => write!(f, ""),
        }
    }
}

pub struct QueryBuilder<'a, C: QueryContext> {
    table: &'a str,
    joins: Option<Vec<C::Join>>,
    where_expr: Option<Expression<C::Query>>,
    sort_expr: Option<SortOrder<C::Sort>>,
    pagination: Option<Pagination>,
}

impl<'a, C> QueryBuilder<'a, C>
where
    C: QueryContext,
{
    pub fn from_ctx() -> Self {
        Self {
            table: C::TABLE,
            joins: None,
            where_expr: None,
            sort_expr: None,
            pagination: None,
        }
    }

    pub fn join(mut self, j: C::Join) -> Self {
        if self.joins.is_none() {
            self.joins = Some(vec![]);
        }

        self.joins.as_mut().unwrap().push(j);
        self
    }

    pub fn r#where(mut self, e: Expression<C::Query>) -> Self {
        self.where_expr = Some(e);
        self
    }

    pub fn order_by(mut self, s: SortOrder<C::Sort>) -> Self {
        self.sort_expr = Some(s);
        self
    }

    pub fn paginate(mut self, p: Pagination) -> Self {
        self.pagination = Some(p);
        self
    }

    pub fn build(self) -> QueryPlan<'a, C> {
        QueryPlan {
            table: self.table,
            joins: self.joins,
            where_expr: self.where_expr,
            sort_expr: self.sort_expr,
            pagination: self.pagination,
        }
    }
}

pub struct QueryPlan<'a, C: QueryContext> {
    joins: Option<Vec<C::Join>>,
    where_expr: Option<Expression<C::Query>>,
    sort_expr: Option<SortOrder<C::Sort>>,
    pagination: Option<Pagination>,
    table: &'a str,
}

impl<'a, C> QueryPlan<'a, C>
where
    C: QueryContext,
    C::Query: Filterable<Entity = C::Model>,
    C::Sort: Sortable<Entity = C::Model>,
{
    fn to_query_builder(&self, build_type: BuildType) -> sqlx::QueryBuilder<'static, Postgres> {
        let head = SqlHead::new(self.table, build_type);
        let mut w = SqlWriter::new(head);

        if let Some(js) = &self.joins {
            w.push_joins(js);
        }

        if let Some(e) = &self.where_expr {
            w.push_where(e);
        }

        if let Some(s) = &self.sort_expr {
            w.push_sort(s);
        }

        if let Some(p) = &self.pagination {
            w.push_pagination(p);
        }

        w.into_builder()
    }

    pub async fn fetch_all<'e, E>(&self, exec: E) -> Result<Vec<C::Model>, sqlx::Error>
    where
        E: sqlx::Executor<'e, Database = Postgres>,
    {
        self.to_query_builder(BuildType::Select(SelectType::Star))
            .build_query_as::<C::Model>()
            .fetch_all(exec)
            .await
    }

    pub async fn fetch_one<'e, E>(&self, exec: E) -> Result<C::Model, sqlx::Error>
    where
        E: sqlx::Executor<'e, Database = Postgres>,
    {
        self.to_query_builder(BuildType::Select(SelectType::Star))
            .build_query_as::<C::Model>()
            .fetch_one(exec)
            .await
    }

    pub async fn fetch_optional<'e, E>(&self, exec: E) -> Result<Option<C::Model>, sqlx::Error>
    where
        E: sqlx::Executor<'e, Database = Postgres>,
    {
        self.to_query_builder(BuildType::Select(SelectType::Star))
            .build_query_as::<C::Model>()
            .fetch_optional(exec)
            .await
    }

    #[cfg(test)]
    pub fn sql(&self, build: BuildType) -> String {
        use sqlx::Execute;

        self.to_query_builder(build).build().sql().to_string()
    }
}

#[macro_export]
macro_rules! and {
    ( $( $e:expr ),* $(,)? ) => {
        $crate::Expression::And(vec![
            $( $crate::Expression::from($e) ),*
        ])
    };
}

#[macro_export]
macro_rules! or {
    ( $( $e:expr ),* $(,)? ) => {
        $crate::Expression::Or(vec![
            $( $crate::Expression::from($e) ),*
        ])
    };
}

#[macro_export]
macro_rules! order_by {
    ( $( $e:expr ),+ $(,)? ) => {
        // Use From<Vec<_>> to avoid constructing the tuple struct directly at call site
        < $crate::SortOrder<_> as ::core::convert::From<::std::vec::Vec<_>> >
            ::from(vec![ $( $e ),+ ])
    };
    () => {
        < $crate::SortOrder<_> as ::core::convert::From<::std::vec::Vec<_>> >
            ::from(::std::vec::Vec::new())
    };
}

#[cfg(test)]
mod tests {
    use claims::assert_some;
    use claims::assert_some_eq;
    use filter_macros::Query;
    use filter_macros::WebQuery;
    use filter_traits::JoinKind;
    use serde::Deserialize;
    use serde::Serialize;
    use serde_json::json;
    use serde_json::Value;
    use sqlx::migrate;
    use sqlx::postgres::PgConnectOptions;
    use sqlx::postgres::PgPoolOptions;
    use sqlx::postgres::PgSslMode;
    use sqlx::types::chrono;
    use sqlx::FromRow;
    use sqlx::PgPool;
    use uuid::Uuid;

    use crate::and;
    use crate::or;
    use crate::repo::ReadRepository;
    use crate::*;

    #[allow(dead_code)]
    #[derive(Debug, FromRow, Clone, Query, PartialEq)]
    pub struct Item {
        #[primary_key]
        id: Uuid,
        name: String,
        description: String,
        price: f32,
        amount: i32,
        active: bool,
        due_date: chrono::DateTime<chrono::Utc>,

        #[foreign_key(to = "material.id")]
        material_id: Option<Uuid>,
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone, WebQuery, Deserialize, Serialize)]
    pub struct ItemDto {
        id: Uuid,
        name: String,
        description: String,
        price: f32,
        amount: i32,
        active: bool,
        due_date: chrono::DateTime<chrono::Utc>,
    }

    #[allow(dead_code)]
    #[derive(Debug, FromRow, Clone, Query)]
    pub struct Material {
        #[primary_key]
        id: Uuid,

        name: String,
        long_name: String,
        description: String,
    }

    pub trait NormalizeString {
        fn normalize(&self) -> String;
    }

    impl NormalizeString for String {
        fn normalize(&self) -> String {
            self.split_whitespace().collect::<Vec<_>>().join(" ")
        }
    }

    impl NormalizeString for &str {
        fn normalize(&self) -> String {
            self.split_whitespace().collect::<Vec<_>>().join(" ")
        }
    }

    struct ItemRepo {}
    impl ReadRepository<Item, ItemQuery, ItemSort> for ItemRepo {
        fn filter(
            &self,
            _e: Expression<ItemQuery>,
            _s: Option<SortOrder<ItemSort>>,
            _p: Pagination,
        ) -> Vec<Item> {
            vec![
                create_test_item(&Uuid::new_v4()),
                create_test_item(&Uuid::new_v4()),
            ]
        }

        fn query(&self, _e: Expression<ItemQuery>) -> Item {
            create_test_item(&Uuid::new_v4())
        }

        fn count(&self, _e: Expression<ItemQuery>) -> usize {
            2
        }

        fn exists(&self, _e: Expression<ItemQuery>) -> bool {
            true
        }
    }

    fn create_test_item(material_id: &Uuid) -> Item {
        Item {
            id: Uuid::new_v4(),
            name: "Test Item".to_string(),
            description: "This is a test item".to_string(),
            price: 9.99,
            amount: 10,
            active: true,
            due_date: chrono::Utc::now(),
            material_id: Some(material_id.clone()),
        }
    }

    #[test]
    fn expression_macros() {
        let plain_query = Expression::Or(vec![
            Expression::And(vec![
                Expression::Leaf(ItemQuery::NameLike("%SternLampe%".into())),
                Expression::Leaf(ItemQuery::DescriptionNeq("Hohlweg".into())),
            ]),
            Expression::Leaf(ItemQuery::PriceGt(1800f32)),
        ]);

        let long_macro_query = or![
            and![
                Expression::Leaf(ItemQuery::NameLike("%SternLampe%".into())),
                Expression::Leaf(ItemQuery::DescriptionNeq("Hohlweg".into())),
            ],
            Expression::Leaf(ItemQuery::PriceGt(1800f32)),
        ];

        let short_macro_query = or![
            and![
                ItemQuery::NameLike("%SternLampe%".into()),
                ItemQuery::DescriptionNeq("Hohlweg".into()),
            ],
            ItemQuery::PriceGt(1800f32),
        ];

        assert_eq!(plain_query, long_macro_query);
        assert_eq!(long_macro_query, short_macro_query);
    }

    #[test]
    fn sort_macros() {
        let plain_sort = SortOrder(vec![ItemSort::ByAmountAsc, ItemSort::ByNameDesc]);
        let short_macro_sort = order_by![ItemSort::ByAmountAsc, ItemSort::ByNameDesc];

        assert_eq!(plain_sort, short_macro_sort);
    }

    #[test]
    fn repository() {
        let e = or![
            and![
                ItemQuery::NameLike("%SternLampe%".into()),
                ItemQuery::DescriptionNeq("Hohlweg".into()),
            ],
            ItemQuery::PriceGt(1800f32),
        ];

        let repo = ItemRepo {};
        let items = repo.filter(e.clone(), None, Pagination::default());
        let count = repo.count(e);

        assert_eq!(items.len(), count);
    }

    #[test]
    fn query_builder() {
        let plan: QueryPlan<Item> = QueryBuilder::from_ctx()
            .join(ItemJoin::ItemToMaterialByMaterialId(JoinKind::Left))
            .r#where(and![
                ItemQuery::NameLike("Clemens".into()),
                or![ItemQuery::PriceGt(1800.00f32), ItemQuery::DescriptionIsNull,]
            ])
            .order_by(order_by![ItemSort::ByNameAsc, ItemSort::ByPriceDesc])
            .paginate(Pagination {
                page: 2,
                page_size: 50,
            })
            .build();

        assert_eq!(
            plan.sql(BuildType::Raw).trim_start(),
            r#"
                LEFT JOIN material ON "item"."material_id" = "material"."id"
                WHERE (name LIKE $1 AND (price > $2 OR description IS NULL))
                ORDER BY name ASC, price DESC
                LIMIT $3 OFFSET $4
            "#
            .normalize()
        )
    }

    #[derive(Clone)]
    pub struct DatabaseSettings {
        pub username: String,
        pub password: String,
        pub port: u16,
        pub host: String,
        pub database_name: String,
        pub require_ssl: bool,
    }

    impl DatabaseSettings {
        pub fn with_db(&self) -> PgConnectOptions {
            let options = self.without_db().database(&self.database_name);
            options
        }

        pub fn without_db(&self) -> PgConnectOptions {
            let pg_ssl_mode = if self.require_ssl {
                PgSslMode::Require
            } else {
                PgSslMode::Prefer
            };
            let ssl_mode = pg_ssl_mode;

            PgConnectOptions::new()
                .host(&self.host)
                .username(&self.username)
                .password(&self.password)
                .port(self.port)
                .ssl_mode(ssl_mode)
        }
    }

    pub async fn get_connection_pool() -> PgPool {
        let mut cfg = DatabaseSettings {
            username: "postgres".into(),
            password: "password".into(),
            port: 2345,
            host: "localhost".into(),
            database_name: "postgres".into(),
            require_ssl: false,
        };

        let server_pool = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(cfg.clone().without_db())
            .await
            .expect("connect server");

        let db_name = Uuid::new_v4().to_string();

        sqlx::query(&format!(r#"CREATE DATABASE "{}""#, db_name))
            .execute(&server_pool)
            .await
            .expect("create db");

        cfg.database_name = db_name.clone();
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect_with(cfg.with_db())
            .await
            .expect("connect new db");

        migrate!("../migrations").run(&pool).await.unwrap();

        pool
    }

    #[tokio::test]
    async fn query_returns_expected_values() {
        let pool = get_connection_pool().await;

        let item = Item {
            id: Uuid::new_v4(),
            name: "test".into(),
            description: "item description".into(),
            price: 23.5f32,
            amount: 2,
            active: true,
            due_date: chrono::Utc::now(),
            material_id: None,
        };

        sqlx::query(
            r#"
            INSERT INTO item (
                id, name, description, price, amount, active, due_date, material_id
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(item.id)
        .bind(&item.name)
        .bind(&item.description)
        .bind(item.price)
        .bind(item.amount)
        .bind(item.active)
        .bind(item.due_date)
        .bind(item.material_id)
        .execute(&pool)
        .await
        .unwrap();

        let maybe: Option<Item> = QueryBuilder::<Item>::from_ctx()
            .r#where(and![
                ItemQuery::NameEq("test".into()),
                or![ItemQuery::PriceLt(10.00f32), ItemQuery::AmountEq(2)]
            ])
            .order_by(order_by![ItemSort::ByNameAsc, ItemSort::ByPriceDesc])
            .paginate(Pagination {
                page: 0,
                page_size: 50,
            })
            .build()
            .fetch_optional(&pool)
            .await
            .unwrap();

        assert_some_eq!(maybe, item);
    }

    #[test]
    fn deserialize_itemdto_filter_json() {
        let json: Value = json!({
            "filter": {
                "and": [
                    { "name": { "like": "%Sternlampe%" } },
                    { "or": [
                        { "price": { "gt": 18.00 } },
                        { "description": { "neq": "von Hohlweg" } }
                    ]}
                ]
            },
            "sort": [
                { "name": "asc" },
                { "description": "desc" }
            ],
            "page": { "pageSize": 10, "pageNo": 1 }
        });

        let f: ItemDtoFilter = serde_json::from_value(json).expect("valid ItemDtoFilter");

        assert_eq!(f.page.page_size, 10);
        assert_eq!(f.page.page_no, 1);
    }
}
