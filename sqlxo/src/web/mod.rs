use serde::{
	Deserialize,
	Serialize,
};
use sqlxo_traits::{
	QueryContext,
	WebJoinPayload,
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
                             Deserialize<'de>, J: WebJoinPayload + \
                             Deserialize<'de>"))]
#[into_params(parameter_in = Query)]
pub struct GenericWebFilter<Q, S, A, J>
where
	Q: WebLeaf + Serialize,
	S: WebSortField + Serialize,
	A: WebLeaf + Serialize,
	J: WebJoinPayload + Serialize,
{
	#[schema(nullable)]
	pub joins:  Option<Vec<JoinPayload<J>>>,
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

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug, IntoParams)]
#[serde(bound(deserialize = "Q: WebLeaf + Deserialize<'de>"))]
#[serde(deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct GenericWebMutationFilter<Q>
where
	Q: WebLeaf + Serialize,
{
	#[schema(no_recursion, nullable)]
	pub filter: Option<GenericWebExpression<Q>>,
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
#[serde(rename_all = "camelCase")]
pub struct WebSearch {
	pub query:        String,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub language:     Option<String>,
	#[serde(default, skip_serializing_if = "Option::is_none")]
	pub include_rank: Option<bool>,
}

pub type WebExpression<T> =
	GenericWebExpression<<T as WebQueryModel>::Leaf>;
pub type WebAggregateExpression<T> =
	GenericWebExpression<<T as WebQueryModel>::AggregateLeaf>;
pub type WebSort<T> = GenericWebSort<<T as WebQueryModel>::SortField>;
pub type WebReadFilter<T> = GenericWebFilter<
	<T as WebQueryModel>::Leaf,
	<T as WebQueryModel>::SortField,
	<T as WebQueryModel>::AggregateLeaf,
	<T as WebQueryModel>::JoinPath,
>;
pub type WebFilter<T> = WebReadFilter<T>;
pub type WebUpdateFilter<T> =
	GenericWebMutationFilter<<T as WebQueryModel>::Leaf>;
pub type WebDeleteFilter<T> =
	GenericWebMutationFilter<<T as WebQueryModel>::Leaf>;

pub trait AggregateBindable<C>: WebQueryModel
where
	C: QueryContext,
{
	fn map_aggregate_leaf(
		leaf: &<Self as WebQueryModel>::AggregateLeaf,
	) -> HavingPredicate;
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
pub enum NoJoins {}

impl WebJoinPayload for NoJoins {
	fn flatten(&self, _prefix: &mut Vec<String>, _out: &mut Vec<Vec<String>>) {
		match *self {}
	}
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
#[serde(transparent)]
pub struct JoinPayload<J>(pub J);

impl<J> JoinPayload<J> {
	pub fn inner(&self) -> &J {
		&self.0
	}
}
