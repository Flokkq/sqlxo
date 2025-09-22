use filter_traits::{Filterable, Sortable};

use crate::{Expression, Page, SortOrder};

pub trait ReadRepository<M /*:Model*/, F: Filterable, S: Sortable> {
    fn filter(&self, e: Expression<F>, s: Option<SortOrder<S>>, p: Page) -> Vec<M>;
    fn query(&self, e: Expression<F>) -> M;
    fn count(&self, e: Expression<F>) -> usize;
    fn exists(&self, e: Expression<F>) -> bool;
}
