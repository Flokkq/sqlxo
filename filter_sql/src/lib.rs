#[cfg(test)]
mod tests {
    use filter_macros::Filter;
    use filter_macros::IntoFilters;
    use filter_traits::Filterable;
    use serde::Deserialize;
    use sqlx::FromRow;
    use validator::Validate;

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

    #[derive(Clone, Debug, PartialEq)]
    enum ItemInputFilter {
        PriceAbove(f64),
        PriceBelow(f64),
        AmountAbove(f64),
        AmountBelow(f64),
        SpecialNotEqual(i32),
    }

    /// API-Query-Struct mit automatischem Mapping
    #[derive(Debug, Deserialize, Validate, IntoFilters)]
    #[serde(rename_all = "camelCase")]
    #[filter_query(target = "ItemInputFilter")]
    struct ItemInputFilterQuery {
        #[validate(range(min = 0.0))]
        price_above: Option<f64>,
        #[validate(range(min = 0.0))]
        price_below: Option<f64>,
        #[validate(range(min = 0.0))]
        amount_above: Option<f64>,
        #[validate(range(min = 0.0))]
        amount_below: Option<f64>,

        // explizites Variant-Override
        #[filter(name = "SpecialNotEqual")]
        #[validate(range(min = 0))]
        special_not_equal: Option<i32>,
    }

    impl Default for ItemInputFilterQuery {
        fn default() -> Self {
            Self {
                price_above: None,
                price_below: None,
                amount_above: None,
                amount_below: None,
                special_not_equal: None,
            }
        }
    }

    #[test]
    fn into_filters_success() {
        let q = ItemInputFilterQuery {
            price_above: Some(10.0),
            amount_above: Some(5.0),
            ..Default::default()
        };

        let filters: Vec<ItemInputFilter> = q.try_into().unwrap();
        let expected = vec![
            ItemInputFilter::PriceAbove(10.0),
            ItemInputFilter::AmountAbove(5.0),
        ];
        assert_eq!(filters, expected);
    }

    #[test]
    fn into_filters_override_variant() {
        let q = ItemInputFilterQuery {
            special_not_equal: Some(42),
            ..Default::default()
        };

        let filters: Vec<ItemInputFilter> = q.try_into().unwrap();
        assert_eq!(filters, vec![ItemInputFilter::SpecialNotEqual(42)]);
    }

    #[test]
    fn into_filters_validation_error() {
        let q = ItemInputFilterQuery {
            price_above: Some(-1.0), // verletzt Range-Validator
            ..Default::default()
        };

        let res: anyhow::Result<Vec<ItemInputFilter>> = q.try_into();
        assert!(
            res.is_err(),
            "negative Werte müssen eine Validation-Fehlermeldung liefern"
        );
    }
}
