#![forbid(unsafe_code)]
extern crate self as sqlxo;

pub use sqlxo_macros::*;
pub use sqlxo_traits::*;

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

pub mod blocks;
pub mod web;

mod builder;
pub use builder::{
	QueryBuilder,
	QueryPlan,
};
