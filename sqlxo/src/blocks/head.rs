use core::fmt;
use std::{
	borrow::Cow,
	fmt::{
		Display,
		Formatter,
	},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum SelectType {
	Star,
	Aggregation(AggregationType),
	StarAndCount,
	Exists,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AggregationType {
	Max,
	Min,
	Count,
	Avg,
}

pub trait ToHead {
	fn to_head(self) -> Cow<'static, str>;
}

pub struct ReadHead<'a> {
	r#type: SelectType,
	table:  &'a str,
}

impl<'a> ReadHead<'a> {
	pub fn new(table: &'a str, r#type: SelectType) -> Self {
		Self { r#type, table }
	}
}

impl<'a> ToHead for ReadHead<'a> {
	fn to_head(self) -> Cow<'static, str> {
		self.to_string().into()
	}
}

impl Display for AggregationType {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		match self {
			AggregationType::Max => f.write_str("MAX"),
			AggregationType::Min => f.write_str("MIN"),
			AggregationType::Count => f.write_str("COUNT"),
			AggregationType::Avg => f.write_str("AVG"),
		}
	}
}

impl<'a> Display for ReadHead<'a> {
	fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
		match &self.r#type {
			SelectType::Star => {
				write!(f, "SELECT * FROM {}", self.table)
			}
			SelectType::Aggregation(agg) => {
				write!(f, "SELECT {}(*) FROM {}", agg, self.table)
			}
			SelectType::StarAndCount => {
				write!(
					f,
					"SELECT *, COUNT(*) OVER() AS total_count FROM {}",
					self.table
				)
			}
			SelectType::Exists => {
				write!(f, "SELECT EXISTS(SELECT 1 FROM {}", self.table)
			} /* #[cfg(any(test, feature = "test-utils"))]
			   * BuildType::Raw => write!(f, ""), */
		}
	}
}
