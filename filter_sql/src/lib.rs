pub mod repo;

#[derive(PartialEq, Debug, Clone)]
pub enum Expression<T /*: Filterable */> {
    And(Vec<Expression<T>>),
    Or(Vec<Expression<T>>),
    Leaf(T),
}

// Wrap raw leaf
impl<T> From<T> for Expression<T> {
    fn from(t: T) -> Self {
        Expression::Leaf(t)
    }
}

pub struct SortOrder<T /*:Sortable*/>(Vec<T>);
pub struct Page {}

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
    #[derive(Debug, FromRow, Query)]
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
}
