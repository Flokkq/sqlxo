use core::fmt;
use std::fmt::{Display, Formatter};

pub enum SelectType {
    Star,
    Aggregation(AggregationType),
}

pub enum AggregationType {
    Max,
    Min,
    Count,
    Avg,
}

pub enum BuildType {
    Select(SelectType),
    Update,
    Delete,
    #[cfg(test)]
    Raw,
}

pub struct SqlHead<'a> {
    build: BuildType,
    table: &'a str,
}

impl<'a> SqlHead<'a> {
    pub fn new(table: &'a str, build: BuildType) -> Self {
        Self { build, table }
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

impl<'a> Display for SqlHead<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.build {
            BuildType::Select(SelectType::Star) => {
                write!(f, "SELECT * FROM {}", self.table)
            }
            BuildType::Select(SelectType::Aggregation(agg)) => {
                write!(f, "SELECT {}(*) FROM {}", agg, self.table)
            }
            BuildType::Update => write!(f, "UPDATE {}", self.table),
            BuildType::Delete => write!(f, "DELETE FROM {}", self.table),
            #[cfg(test)]
            BuildType::Raw => write!(f, ""),
        }
    }
}
