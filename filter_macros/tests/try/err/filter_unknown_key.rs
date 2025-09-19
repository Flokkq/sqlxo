use filter_macros::Query;

#[derive(Query)]
#[filter(foo = "bar")]
pub struct T {
    pub name: String,
}
fn main() {}
