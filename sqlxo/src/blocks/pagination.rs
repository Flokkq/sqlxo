#[derive(Debug, Clone, Copy)]
pub struct Pagination {
	/// Offset in pages to start from
	pub page: i64,

	/// Maximum number of elements to return  
	pub page_size: i64,
}

impl Default for Pagination {
	fn default() -> Self {
		Self::all()
	}
}

impl Pagination {
	fn all() -> Self {
		Pagination {
			page:      0,
			page_size: i32::MAX as i64,
		}
	}
}
