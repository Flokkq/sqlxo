use smallvec::SmallVec;
use sqlx::{
	Postgres,
	QueryBuilder,
};
use std::marker::PhantomData;

use sqlxo_traits::QueryModel;

/// Marker trait for model columns that can participate in `take!`.
pub trait Column: Copy {
	type Model: QueryModel;
	type Type;
	const NAME: &'static str;
}

#[derive(Debug, Clone)]
pub struct SelectionList<Model, Output> {
	pub(crate) columns: SmallVec<[&'static str; 4]>,
	_marker:            PhantomData<(Model, Output)>,
}

impl<Model, Output> SelectionList<Model, Output> {
	pub fn new(columns: SmallVec<[&'static str; 4]>) -> Self {
		Self {
			columns,
			_marker: PhantomData,
		}
	}

	pub fn columns(&self) -> &[&'static str] {
		&self.columns
	}

	pub fn clone_columns(&self) -> SmallVec<[&'static str; 4]> {
		self.columns.clone()
	}

	pub fn push_returning(
		&self,
		qb: &mut QueryBuilder<'static, Postgres>,
		table: &str,
	) {
		qb.push(" RETURNING ");
		for (idx, col) in self.columns.iter().enumerate() {
			if idx > 0 {
				qb.push(", ");
			}
			qb.push(&format!(r#""{}"."{}""#, table, col));
		}
	}
}

pub fn push_returning<M, Output>(
	qb: &mut QueryBuilder<'static, Postgres>,
	table: &str,
	selection: Option<&SelectionList<M, Output>>,
) {
	if let Some(sel) = selection {
		sel.push_returning(qb, table);
	} else {
		qb.push(" RETURNING *");
	}
}

#[doc(hidden)]
#[macro_export]
macro_rules! take {
	($first:path $(, $rest:path)* $(,)?) => {{
		use $crate::select::{
			Column as __SqlxoColumn,
			SelectionList as __SqlxoSelectionList,
		};
		type __SqlxoModel = <$first as __SqlxoColumn>::Model;
		$(let _: ::core::marker::PhantomData<__SqlxoModel> =
			::core::marker::PhantomData::<<$rest as __SqlxoColumn>::Model>; )*

		let mut __cols: smallvec::SmallVec<[&'static str; 4]> =
			smallvec::SmallVec::new();
		__cols.push(<$first as __SqlxoColumn>::NAME);
		$(__cols.push(<$rest as __SqlxoColumn>::NAME);)*

		__SqlxoSelectionList::<
			__SqlxoModel,
			(
				<$first as __SqlxoColumn>::Type,
				$(<$rest as __SqlxoColumn>::Type,)*
			)
		>::new(__cols)
	}};
}
