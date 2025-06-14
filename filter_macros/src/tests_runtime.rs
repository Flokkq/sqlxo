#[cfg(test)]
mod runtime {
    use super::*;
    use once_cell::sync::Lazy;

    /// Dummy-Entity, kein sqlx-Pool nötig
    #[derive(Clone, Debug, PartialEq)]
    struct Row;

    #[derive(Filter, Clone, Debug)]
    #[filter(table = "demo", entity = "Row")]
    enum Demo {
        IdEq(i32),
        NameLike(String),
        DeletedIsNull,
    }

    static FILTERS: Lazy<Vec<Demo>> = Lazy::new(|| {
        vec![
            Demo::IdEq(7),
            Demo::DeletedIsNull,
            Demo::NameLike("foo%".into()),
        ]
    });

    #[test]
    fn clause_generation() {
        let c0 = Demo::IdEq(42).filter_clause(1);
        assert_eq!(c0, "id = $1");

        let c1 = Demo::DeletedIsNull.filter_clause(3);
        assert_eq!(c1, "deleted IS NULL");

        let where_clause = FILTERS
            .iter()
            .enumerate()
            .map(|(i, f)| f.filter_clause(i + 1))
            .collect::<Vec<_>>()
            .join(" AND ");

        assert_eq!(where_clause, "id = $1 AND deleted IS NULL AND name LIKE $3");
    }

    /// Stubs für sqlx-freie Bind-Tests:
    mod stub {
        pub struct Query {
            binds: usize,
        }
        impl Query {
            pub fn new() -> Self {
                Self { binds: 0 }
            }
            pub fn bind<T>(mut self, _t: T) -> Self {
                self.binds += 1;
                self
            }
            pub fn binds(&self) -> usize {
                self.binds
            }
        }
    }

    #[test]
    fn bind_count() {
        let q = stub::Query::new();
        let q = Demo::IdEq(1).bind(q);
        let q = Demo::DeletedIsNull.bind(q);
        assert_eq!(q.binds(), 1); // nur IdEq erzeugt $-Bind
    }
}
