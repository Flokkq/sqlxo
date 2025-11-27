use crate::{
	QueryContext,
	WebQueryModel,
};

pub trait Bind<C>: WebQueryModel
where
	C: QueryContext,
{
	fn map_leaf(leaf: &<Self as WebQueryModel>::Leaf) -> C::Query;

	fn map_sort_field(sort: &<Self as WebQueryModel>::SortField) -> C::Sort;
}
