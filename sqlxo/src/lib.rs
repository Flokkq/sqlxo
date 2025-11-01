#![forbid(unsafe_code)]
extern crate self as sqlxo;

pub use sqlxo_traits::{
	Bind,
	DtoExpression,
	DtoFilter,
	DtoPage,
	DtoSort,
	DtoSortDir,
	Filterable,
	JoinKind,
	QueryContext,
	QueryModel,
	Sortable,
	SqlJoin,
	SqlWrite,
	WebQueryModel,
};

pub use sqlxo_macros::{
	bind,
	context,
	Query,
	WebQuery,
};

pub mod prelude {
	pub use super::{
		Filterable,
		JoinKind,
		Query,
		QueryContext,
		Sortable,
		SqlJoin,
		WebQuery,
	};
}

mod builder;
mod expression;
mod head;
mod macros;
mod pagination;
mod sort;
mod webfilter;
mod writer;

pub use builder::{
	QueryBuilder,
	QueryPlan,
};
pub use expression::Expression;
pub use head::{
	AggregationType,
	BuildType,
	SelectType,
	SqlHead,
};
pub use pagination::Pagination;
pub use sort::SortOrder;
pub use writer::SqlWriter;

#[cfg(test)]
mod tests;
