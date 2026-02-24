use serde::{
	Deserialize,
	Serialize,
};
use sqlxo_traits::{
	QueryContext,
	WebLeaf,
	WebQueryModel,
	WebSortField,
};
use utoipa::{
	IntoParams,
	ToSchema,
};

use crate::select::HavingPredicate;

mod builder;
mod page;
pub use page::{
	WebPage,
	WebPagination,
};

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug, IntoParams)]
#[serde(bound(deserialize = "Q: WebLeaf + Deserialize<'de>, S: \
                             WebSortField + Deserialize<'de>, A: WebLeaf + \
                             Deserialize<'de>"))]
#[into_params(parameter_in = Query)]
pub struct GenericWebFilter<Q, S, A>
where
	Q: WebLeaf + Serialize,
	S: WebSortField + Serialize,
	A: WebLeaf + Serialize,
{
	#[schema(nullable)]
	pub joins:  Option<Vec<WebJoinPath>>,
	#[schema(no_recursion, nullable)]
	pub filter: Option<GenericWebExpression<Q>>,
	#[schema(no_recursion, nullable)]
	pub having: Option<GenericWebExpression<A>>,
	#[schema(no_recursion, nullable)]
	pub sort:   Option<Vec<GenericWebSort<S>>>,
	#[schema(no_recursion, nullable)]
	pub search: Option<WebSearch>,
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

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
pub struct WebJoinPath {
	pub path: Vec<String>,
	pub kind: WebJoinKind,
}

#[derive(
	Clone, Copy, Serialize, Deserialize, ToSchema, Debug, PartialEq, Eq,
)]
#[serde(rename_all = "lowercase")]
pub enum WebJoinKind {
	Inner,
	Left,
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
#[serde(rename_all = "camelCase")]
pub struct WebSearch {
	pub query:        String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub language:     Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub include_rank: Option<bool>,
}

pub type WebExpression<T> = GenericWebExpression<<T as WebQueryModel>::Leaf>;
pub type WebAggregateExpression<T> =
	GenericWebExpression<<T as WebQueryModel>::AggregateLeaf>;
pub type WebSort<T> = GenericWebSort<<T as WebQueryModel>::SortField>;
pub type WebFilter<T> = GenericWebFilter<
	<T as WebQueryModel>::Leaf,
	<T as WebQueryModel>::SortField,
	<T as WebQueryModel>::AggregateLeaf,
>;

pub trait AggregateBindable<C>: WebQueryModel
where
	C: QueryContext,
{
	fn map_aggregate_leaf(
		leaf: &<Self as WebQueryModel>::AggregateLeaf,
	) -> HavingPredicate;
}
