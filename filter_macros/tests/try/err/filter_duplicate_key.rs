use filter_macros::Query;

#[derive(Query)]
#[filter(table_name = "a", table_name = "b")]
pub struct T {
    pub name: String,
}
fn main() {}
