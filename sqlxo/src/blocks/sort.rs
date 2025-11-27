use sqlxo_traits::Sortable;

#[macro_export]
macro_rules! order_by {
    ( $( $e:expr ),+ $(,)? ) => {{
        let mut v = ::std::vec::Vec::new();
        $(
            v.extend($e.into_iter());
        )+
        <$crate::blocks::SortOrder<_> as ::core::convert::From<::std::vec::Vec<_>>>
            ::from(v)
    }};
    () => {
        <$crate::blocks::SortOrder<_> as ::core::convert::From<::std::vec::Vec<_>>>
            ::from(::std::vec::Vec::new())
    };
}

#[derive(PartialEq, Debug, Clone)]
pub struct SortOrder<T: Sortable>(pub Vec<T>);

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
