use filter_macros::Query;

#[derive(Debug, Clone, Query)]
#[filter(table_name = "a")]
pub struct T {
    pub name: String,
}
fn main() {}
