use filter_macros::Query;

#[derive(Query)]
pub struct SnakeCaseName {
    pub name: String,
}

fn main() {
    assert_eq!(SNAKE_CASE_NAME_TABLE, "snake_case_name");
}
