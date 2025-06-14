#[cfg(test)]
mod tests {
    use filter_macros::Filter;
    use filter_traits::Filterable;
    use sqlx::FromRow;

    #[derive(Debug, FromRow)]
    struct Row {
        id: i32,
        name: String,
    }

    #[derive(Filter, Clone, Debug, PartialEq)]
    #[filter(table = "demo", entity = "Row")]
    enum Demo {
        IdEq(i32),
        NameLike(String),
        DeletedIsNull,

        #[filter(name = "custom_column", op = "neq")]
        SpecialNotEqual(i32),

        #[filter(name = "x", op = "gt")]
        ExplicitGreaterThan(i32),

        #[filter(name = "flags", op = "is_null")]
        ExplicitIsNull,

        AmountAbove(i64),
        TimestampNotNull,
    }

    #[test]
    fn test_filter_clause_all_variants() {
        let clauses = vec![
            Demo::IdEq(5).filter_clause(1),
            Demo::NameLike("abc%".into()).filter_clause(2),
            Demo::DeletedIsNull.filter_clause(3),
            Demo::SpecialNotEqual(42).filter_clause(4),
            Demo::ExplicitGreaterThan(99).filter_clause(5),
            Demo::ExplicitIsNull.filter_clause(6),
            Demo::AmountAbove(1000).filter_clause(7),
            Demo::TimestampNotNull.filter_clause(8),
        ];

        let expected = vec![
            "id = $1",
            "name LIKE $2",
            "deleted IS NULL",
            "custom_column <> $4",
            "x > $5",
            "flags IS NULL",
            "amount > $7",
            "timestamp IS NOT NULL",
        ];

        assert_eq!(clauses, expected);
    }

    #[test]
    fn test_to_sql_with_complex_filters() {
        let filters = vec![
            Demo::IdEq(1),
            Demo::SpecialNotEqual(2),
            Demo::AmountAbove(10),
            Demo::TimestampNotNull,
        ];

        let expected = "SELECT * FROM demo WHERE id = $1 AND custom_column <> $2 AND amount > $3 AND timestamp IS NOT NULL";
        let actual = Demo::to_sql(&filters);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_to_sql_with_no_filters() {
        let filters: Vec<Demo> = vec![];
        let expected = "SELECT * FROM demo";
        let actual = Demo::to_sql(&filters);

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_table_name_constant() {
        assert_eq!(Demo::table_name(), "demo");
    }
}
