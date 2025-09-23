use filter_traits::{Filterable, QueryContext, Sortable};
use sqlx::{postgres::PgArguments, Postgres};

pub mod repo;

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
    pub fn to_sql(&self, idx: &mut usize) -> String {
        match self {
            Expression::Leaf(q) => q.filter_clause(idx),
            Expression::And(xs) => {
                let parts: Vec<String> = xs.iter().map(|x| x.to_sql(idx)).collect();
                format!("({})", parts.join(" AND "))
            }
            Expression::Or(xs) => {
                let parts: Vec<String> = xs.iter().map(|x| x.to_sql(idx)).collect();
                format!("({})", parts.join(" OR "))
            }
        }
    }
}

impl<T> Expression<T>
where
    T: Filterable + Clone,
{
    pub fn bind_into<'q>(
        &self,
        mut q: sqlx::query::QueryAs<'q, sqlx::Postgres, T::Entity, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::QueryAs<'q, sqlx::Postgres, T::Entity, sqlx::postgres::PgArguments> {
        match self {
            Self::Leaf(f) => f.clone().bind(q),
            Self::And(xs) | Self::Or(xs) => {
                for x in xs {
                    q = x.bind_into(q);
                }
                q
            }
        }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub struct SortOrder<T: Sortable>(Vec<T>);

impl<T> SortOrder<T>
where
    T: Sortable + Clone,
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

pub struct Page {}

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
}

pub struct QueryBuilder<'a, C: QueryContext> {
    table: &'a str,
    where_expr: Option<Expression<C::Query>>,
    sort_expr: Option<SortOrder<C::Sort>>,
}

impl<'a, C> QueryBuilder<'a, C>
where
    C: QueryContext,
{
    pub fn from_ctx() -> Self {
        Self {
            table: C::TABLE,
            where_expr: None,
            sort_expr: None,
        }
    }

    pub fn r#where(mut self, e: Expression<C::Query>) -> Self {
        self.where_expr = Some(e);
        self
    }

    pub fn order_by(mut self, s: SortOrder<C::Sort>) -> Self {
        self.sort_expr = Some(s);
        self
    }

    fn to_sql(&self) -> String {
        let mut idx = 0;

        let where_sql = match &self.where_expr {
            Some(e) => format!(" WHERE {}", e.to_sql(&mut idx)),
            None => String::new(),
        };

        let sort_sql = match &self.sort_expr {
            Some(s) => format!(" ORDER BY {}", s.to_sql()),
            None => String::new(),
        };

        format!("{}{}", where_sql, sort_sql)
    }

    pub fn build(self) -> BuiltQuery<'a, C> {
        // sql.push_str(&self.to_sql());

        BuiltQuery {
            table: self.table,
            sql: self.to_sql(),
            where_expr: self.where_expr,
            sort_expr: self.sort_expr,
        }
    }
}

pub struct BuiltQuery<'a, C: QueryContext> {
    sql: String,
    where_expr: Option<Expression<C::Query>>,
    sort_expr: Option<SortOrder<C::Sort>>,
    table: &'a str,
}

impl<'a, C> BuiltQuery<'a, C>
where
    C: QueryContext,
    C::Query: Filterable<Entity = C::Model>,
    C::Sort: Sortable<Entity = C::Model>,
{
    pub fn as_query(
        &self,
        build_type: BuildType,
    ) -> sqlx::query::QueryAs<'_, Postgres, C::Model, PgArguments> {
        let mut qb = sqlx::QueryBuilder::<Postgres>::new(match build_type {
            BuildType::Select(_) => format!("SELECT * FROM {}", self.table),
            BuildType::Update => format!("UPDATE {}", self.table),
            BuildType::Delete => format!("DELETE FROM {}", self.table),
        });
        qb.push(self.sql.as_str());

        let mut q = qb.build_query_as::<C::Model>();

        if let Some(w) = &self.where_expr {
            q = w.bind_into(q);
        }

        q
    }

    pub async fn fetch_all<'e, E>(&self, exec: E) -> Result<Vec<C::Model>, sqlx::Error>
    where
        E: sqlx::Executor<'e, Database = Postgres>,
    {
        self.as_query(BuildType::Select(SelectType::Star))
            .fetch_all(exec)
            .await
    }

    pub async fn fetch_one<'e, E>(&self, exec: E) -> Result<C::Model, sqlx::Error>
    where
        E: sqlx::Executor<'e, Database = Postgres>,
    {
        self.as_query(BuildType::Select(SelectType::Star))
            .fetch_one(exec)
            .await
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
    use filter_macros::Query;
    use sqlx::types::chrono;
    use sqlx::FromRow;
    use uuid::Uuid;

    use crate::and;
    use crate::or;
    use crate::repo::ReadRepository;
    use crate::*;

    #[allow(dead_code)]
    #[derive(Debug, FromRow, Clone, Query)]
    pub struct Item {
        id: Uuid,
        name: String,
        description: String,
        price: f32,
        amount: i32,
        active: bool,
        due_date: chrono::DateTime<chrono::Utc>,
    }

    struct ItemRepo {}
    impl ReadRepository<Item, ItemQuery, ItemSort> for ItemRepo {
        fn filter(
            &self,
            _e: Expression<ItemQuery>,
            _s: Option<SortOrder<ItemSort>>,
            _p: Page,
        ) -> Vec<Item> {
            vec![create_test_item(), create_test_item()]
        }

        fn query(&self, _e: Expression<ItemQuery>) -> Item {
            create_test_item()
        }

        fn count(&self, _e: Expression<ItemQuery>) -> usize {
            2
        }

        fn exists(&self, _e: Expression<ItemQuery>) -> bool {
            true
        }
    }

    fn create_test_item() -> Item {
        Item {
            id: Uuid::new_v4(),
            name: "Test Item".to_string(),
            description: "This is a test item".to_string(),
            price: 9.99,
            amount: 10,
            active: true,
            due_date: chrono::Utc::now(),
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
        let items = repo.filter(e.clone(), None, Page {});
        let count = repo.count(e);

        assert_eq!(items.len(), count);
    }

    #[test]
    fn query_builder() {
        let built: BuiltQuery<Item> = QueryBuilder::from_ctx()
            .r#where(and![
                ItemQuery::NameLike("Clemens".into()),
                or![ItemQuery::PriceGt(1800.00f32), ItemQuery::DescriptionIsNull,]
            ])
            .order_by(order_by![ItemSort::ByNameAsc, ItemSort::ByPriceDesc])
            .build();

        assert_eq!(
            built.sql,
            "WHERE (name LIKE $1 AND (price > $2 OR description IS NULL)) ORDER BY name ASC, price DESC"
        )
    }
}
