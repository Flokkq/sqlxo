use filter_traits::{Filterable, Sortable};

pub mod repo;
pub mod testss;

pub enum Expression<T: Filterable> {
    And(Vec<Expression<T>>),
    Or(Vec<Expression<T>>),
    Leaf(T),
}

pub struct SortOrder<T: Sortable>(Vec<T>);

pub struct Page {}
