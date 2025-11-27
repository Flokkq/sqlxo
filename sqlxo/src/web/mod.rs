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

mod builder;
mod page;
pub use page::WebPagination;

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug, IntoParams)]
#[serde(bound(deserialize = "Q: WebLeaf + Deserialize<'de>, S: \
                             WebSortField + Deserialize<'de>"))]
#[into_params(parameter_in = Query)]
pub struct GenericWebFilter<Q, S>
where
	Q: WebLeaf + Serialize,
	S: WebSortField + Serialize,
{
	#[schema(no_recursion, nullable)]
	pub filter: Option<GenericWebExpression<Q>>,
	#[schema(no_recursion, nullable)]
	pub sort:   Option<Vec<GenericWebSort<S>>>,
	#[schema(nullable)]
	pub page:   Option<WebPagination>,
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
#[serde(bound(deserialize = "Q: WebLeaf + Deserialize<'de>"))]
#[serde(untagged)]
pub enum GenericWebExpression<Q>
where
	Q: WebLeaf + Serialize,
{
	#[schema(no_recursion)]
	And {
		and: Vec<GenericWebExpression<Q>>,
	},
	#[schema(no_recursion)]
	Or {
		or: Vec<GenericWebExpression<Q>>,
	},
	Leaf(Q),
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
#[serde(bound(deserialize = "S: WebSortField + Deserialize<'de>"))]
#[serde(transparent)]
pub struct GenericWebSort<S>(pub S)
where
	S: WebSortField + Serialize;

pub type WebExpression<T> = GenericWebExpression<<T as WebQueryModel>::Leaf>;
pub type WebSort<T> = GenericWebSort<<T as WebQueryModel>::SortField>;
pub type WebFilter<T> = GenericWebFilter<
	<T as WebQueryModel>::Leaf,
	<T as WebQueryModel>::SortField,
>;
