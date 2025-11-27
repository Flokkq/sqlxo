use serde::{
	Deserialize,
	Serialize,
};
use utoipa::{
	IntoParams,
	ToSchema,
};

use crate::blocks::Page;

/// Standard pagination sent as **query** parameters.
///
/// *If the caller omits either field Axum fills in the defaults.*
#[derive(Deserialize, Serialize, Debug, Clone, Copy, IntoParams, ToSchema)]
#[serde(default, rename_all = "camelCase")]
pub struct WebPagination {
	/// Offset in pages to start from
	#[serde(deserialize_with = "non_negative_i64", rename = "pageNo")]
	#[param(example = 0)]
	pub page: i64,

	/// Maximum number of elements to return  
	#[serde(deserialize_with = "positive_i64")]
	#[param(example = 10)]
	pub page_size: i64,
}

fn non_negative_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
	D: serde::Deserializer<'de>,
{
	let v = i64::deserialize(deserializer)?;
	if v < 0 {
		Err(serde::de::Error::custom("offset must be >= 0"))
	} else {
		Ok(v)
	}
}

fn positive_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
	D: serde::Deserializer<'de>,
{
	let v = i64::deserialize(deserializer)?;
	if v <= 0 {
		Err(serde::de::Error::custom("page_size must be > 0"))
	} else {
		Ok(v)
	}
}

impl Default for WebPagination {
	fn default() -> Self {
		WebPagination {
			page:      0,
			page_size: i32::MAX as i64,
		}
	}
}

/// Standard pagination response.
#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebPage<T> {
	pub items:       Vec<T>,
	pub page_size:   i64,
	pub page:        i64,
	pub total:       i64,
	pub total_pages: i64,
}

impl<T> From<Page<T>> for WebPage<T> {
	fn from(value: Page<T>) -> Self {
		Self {
			items:       value.items,
			page_size:   value.page_size,
			page:        value.page,
			total:       value.total,
			total_pages: value.total_pages,
		}
	}
}
