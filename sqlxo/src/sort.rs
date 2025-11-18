use sqlxo_traits::Sortable;

#[derive(PartialEq, Debug, Clone)]
pub struct SortOrder<T: Sortable>(pub(crate) Vec<T>);

impl<T> SortOrder<T>
where
	T: Sortable,
{
	pub fn to_sql(&self) -> String {
		let mut out = String::new();

		for (i, s) in self.0.iter().enumerate() {
			if i > 0 {
				out.push_str(", ");
			}
			out.push_str(&s.sort_clause());
		}

		out
	}
}

impl<T: Sortable> From<Vec<T>> for SortOrder<T> {
	fn from(v: Vec<T>) -> Self {
		Self(v)
	}
}

impl<T: Sortable> From<SortOrder<T>> for Vec<T> {
	fn from(value: SortOrder<T>) -> Self {
		value.0
	}
}

impl<T: Sortable> std::iter::IntoIterator for SortOrder<T> {
	type Item = T;
	type IntoIter = ::std::vec::IntoIter<T>;

	fn into_iter(self) -> Self::IntoIter {
		self.0.into_iter()
	}
}
