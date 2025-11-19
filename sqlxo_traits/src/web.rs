use serde::{
	Deserialize,
	Serialize,
};
use utoipa::ToSchema;

pub trait WebQueryModel {
	type Leaf: ToSchema
		+ Clone
		+ serde::Serialize
		+ for<'de> serde::Deserialize<'de>;
	type SortField: ToSchema
		+ Clone
		+ serde::Serialize
		+ for<'de> serde::Deserialize<'de>;
}

pub trait WebSortAccess: WebQueryModel {
	fn sort_field(s: &DtoSort<Self>) -> <Self as WebQueryModel>::SortField;
	fn sort_dir(s: &DtoSort<Self>) -> DtoSortDir;
}

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
#[serde(untagged)]
pub enum GenericDtoExpression<Q> {
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
#[schema(bound = "S: ToSchema")]
#[serde(transparent)]
pub struct GenericDtoSort<S>(pub S);

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

#[derive(Clone, Serialize, Deserialize, ToSchema, Debug)]
#[schema(bound = "Q: ToSchema, S: ToSchema")]
pub struct GenericDtoFilter<Q, S> {
	#[schema(no_recursion)]
	pub filter: Option<GenericDtoExpression<Q>>,
	#[schema(no_recursion)]
	pub sort:   Option<Vec<GenericDtoSort<S>>>,
	pub page:   Option<DtoPage>,
}

pub type DtoExpression<T> = GenericDtoExpression<<T as WebQueryModel>::Leaf>;

pub type DtoSort<T> = GenericDtoSort<<T as WebQueryModel>::SortField>;

pub type DtoFilter<T> = GenericDtoFilter<
	<T as WebQueryModel>::Leaf,
	<T as WebQueryModel>::SortField,
>;
