use crate::writer::SqlWriter;
use sqlo_traits::{
	Filterable,
	SqlWrite,
};

#[derive(PartialEq, Debug, Clone)]
pub enum Expression<T: Filterable> {
	And(Vec<Expression<T>>),
	Or(Vec<Expression<T>>),
	Leaf(T),
}

impl<T> From<T> for Expression<T>
where
	T: Filterable,
{
	fn from(t: T) -> Self {
		Expression::Leaf(t)
	}
}

impl<T: Filterable> Expression<T> {
	pub fn write(&self, w: &mut SqlWriter) {
		match self {
			Expression::Leaf(q) => q.write(w),
			Expression::And(xs) => {
				w.push("(");
				for (i, x) in xs.iter().enumerate() {
					if i > 0 {
						w.push(" AND ");
					}
					x.write(w);
				}
				w.push(")");
			}
			Expression::Or(xs) => {
				w.push("(");
				for (i, x) in xs.iter().enumerate() {
					if i > 0 {
						w.push(" OR ");
					}
					x.write(w);
				}
				w.push(")");
			}
		}
	}
}
