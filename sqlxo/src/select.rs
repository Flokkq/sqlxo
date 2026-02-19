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
	const TABLE: &'static str;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionColumn {
	pub table:  &'static str,
	pub column: &'static str,
}

impl SelectionColumn {
	pub const fn new(table: &'static str, column: &'static str) -> Self {
		Self { table, column }
	}
}

#[derive(Debug, Clone)]
pub struct SelectionList<Output> {
	pub(crate) columns: SmallVec<[SelectionColumn; 4]>,
	_marker:            PhantomData<Output>,
}

impl<Output> SelectionList<Output> {
	pub fn new(columns: SmallVec<[SelectionColumn; 4]>) -> Self {
		Self {
			columns,
			_marker: PhantomData,
		}
	}

	pub fn columns(&self) -> &[SelectionColumn] {
		&self.columns
	}

	pub fn clone_columns(&self) -> SmallVec<[SelectionColumn; 4]> {
		self.columns.clone()
	}

	pub fn push_returning(
		&self,
		qb: &mut QueryBuilder<'static, Postgres>,
		table: &str,
	) {
		qb.push(" RETURNING ");
		for (idx, col) in self.columns.iter().enumerate() {
			assert_eq!(
				col.table, table,
				"`RETURNING` may only use columns from `{}` but got `{}`",
				table, col.table,
			);

			if idx > 0 {
				qb.push(", ");
			}
			qb.push(&format!(r#""{}"."{}""#, table, col.column));
		}
	}
}

pub fn push_returning<Output>(
	qb: &mut QueryBuilder<'static, Postgres>,
	table: &str,
	selection: Option<&SelectionList<Output>>,
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
			SelectionColumn as __SqlxoSelectionColumn,
			SelectionList as __SqlxoSelectionList,
		};

		let mut __cols: smallvec::SmallVec<[__SqlxoSelectionColumn; 4]> =
			smallvec::SmallVec::new();
		__cols.push(__SqlxoSelectionColumn::new(
			<$first as __SqlxoColumn>::TABLE,
			<$first as __SqlxoColumn>::NAME,
		));
		$(__cols.push(__SqlxoSelectionColumn::new(
			<$rest as __SqlxoColumn>::TABLE,
			<$rest as __SqlxoColumn>::NAME,
		));)*

		__SqlxoSelectionList::<
			(
				<$first as __SqlxoColumn>::Type,
				$(<$rest as __SqlxoColumn>::Type,)*
			)
		>::new(__cols)
	}};
}
