use serde::{
	Deserialize,
	Serialize,
};
use utoipa::{
	PartialSchema,
	ToSchema,
};

pub trait WebLeaf: Clone + Serialize + ToSchema + PartialSchema {}
impl<T> WebLeaf for T where
	T: Clone + Serialize + for<'de> Deserialize<'de> + ToSchema + PartialSchema
{
}

pub trait WebSortField: Clone + Serialize + ToSchema + PartialSchema {}
impl<T> WebSortField for T where
	T: Clone + Serialize + for<'de> Deserialize<'de> + ToSchema + PartialSchema
{
}

pub trait WebQueryModel {
	type Leaf: WebLeaf;
	type SortField: WebSortField;
}
