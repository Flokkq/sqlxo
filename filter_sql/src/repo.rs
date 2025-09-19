use filter_traits::{Filterable, Model, Sortable};

use crate::{Expression, Page, SortOrder};

pub trait ReadRepository<M: Model, F: Filterable, S: Sortable> {
    fn filter(e: Expression<F>, s: Option<SortOrder<S>>, p: Page) -> Vec<M>;
    fn query(e: Expression<F>) -> M;
    fn count(e: Expression<F>) -> i32;
    fn exists(e: Expression<F>) -> bool;
}
