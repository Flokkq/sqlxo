use filter_macros::Query;

#[derive(Query)]
#[filter(table_name = 123)]
pub struct T {
    pub name: String,
}
fn main() {}
