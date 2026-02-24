use crate::QueryContext;
use serde::{
	Deserialize,
	Serialize,
};
use utoipa::{
	PartialSchema,
	ToSchema,
};

pub trait WebLeaf:
	Clone + Send + Sync + Serialize + ToSchema + PartialSchema
{
}

impl<T> WebLeaf for T where
	T: Clone
		+ Send
		+ Sync
		+ Serialize
		+ for<'de> Deserialize<'de>
		+ ToSchema
		+ PartialSchema
{
}

pub trait WebSortField:
	Clone + Send + Sync + Serialize + ToSchema + PartialSchema
{
}
impl<T> WebSortField for T where
	T: Clone
		+ Serialize
		+ for<'de> Deserialize<'de>
		+ ToSchema
		+ PartialSchema
		+ Send
		+ Sync
{
}

pub trait WebJoinPayload:
	Clone + Send + Sync + Serialize + ToSchema + PartialSchema
{
	fn flatten(&self, prefix: &mut Vec<String>, out: &mut Vec<Vec<String>>);
}

pub trait WebQueryModel {
	type Leaf: WebLeaf;
	type SortField: WebSortField;
	type AggregateLeaf: WebLeaf;
	type JoinPath: WebJoinPayload + for<'de> Deserialize<'de>;
}

pub trait Bind<C>: WebQueryModel
where
	C: QueryContext,
{
	fn map_leaf(leaf: &<Self as WebQueryModel>::Leaf) -> C::Query;

	fn map_sort_field(sort: &<Self as WebQueryModel>::SortField) -> C::Sort;
}

#[derive(Clone, Copy, Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "lowercase")]
pub enum WebSortDirection {
	Asc,
	Desc,
}
