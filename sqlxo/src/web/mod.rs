use serde::{
	Deserialize,
	Serialize,
};
use sqlxo_traits::{
	WebLeaf,
	WebQueryModel,
	WebSortField,
};
use utoipa::{
	IntoParams,
	ToSchema,
};

mod webfilter;

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
#[serde(bound(deserialize = "Q: WebLeaf + Deserialize<'de>"))]
#[serde(untagged)]
pub enum GenericDtoExpression<Q>
where
	Q: WebLeaf + Serialize,
{
	#[schema(no_recursion)]
	And {
		and: Vec<GenericDtoExpression<Q>>,
	},
	#[schema(no_recursion)]
	Or {
		or: Vec<GenericDtoExpression<Q>>,
	},
	Leaf(Q),
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
#[serde(bound(deserialize = "S: WebSortField + Deserialize<'de>"))]
#[serde(transparent)]
pub struct GenericDtoSort<S>(pub S)
where
	S: WebSortField + Serialize;

#[derive(Clone, Copy, Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "lowercase")]
pub enum DtoSortDir {
	Asc,
	Desc,
}

#[derive(Clone, Copy, Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DtoPage {
	pub page_size: u32,
	pub page_no:   u32,
}

impl Default for DtoPage {
	fn default() -> Self {
		Self {
			page_size: u32::MAX,
			page_no:   0,
		}
	}
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug, IntoParams)]
#[serde(bound(deserialize = "Q: WebLeaf + Deserialize<'de>, S: \
                             WebSortField + Deserialize<'de>"))]
#[into_params(parameter_in = Query)]
pub struct GenericDtoFilter<Q, S>
where
	Q: WebLeaf + Serialize,
	S: WebSortField + Serialize,
{
	#[schema(no_recursion, nullable)]
	pub filter: Option<GenericDtoExpression<Q>>,
	#[schema(no_recursion, nullable)]
	pub sort:   Option<Vec<GenericDtoSort<S>>>,
	#[schema(nullable)]
	pub page:   Option<DtoPage>,
}

pub type DtoExpression<T> = GenericDtoExpression<<T as WebQueryModel>::Leaf>;
pub type DtoSort<T> = GenericDtoSort<<T as WebQueryModel>::SortField>;
pub type DtoFilter<T> = GenericDtoFilter<
	<T as WebQueryModel>::Leaf,
	<T as WebQueryModel>::SortField,
>;
