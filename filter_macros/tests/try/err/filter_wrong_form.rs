use filter_macros::Query;

#[derive(Query)]
#[filter = "oops"]
pub struct T {
    pub name: String,
}
fn main() {}
