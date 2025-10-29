use sqlo_macros::Query;

#[derive(Debug, Clone, Query)]
#[sqlo(table_name = "a")]
pub struct T {
    pub name: String,
}
fn main() {}
