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

/// Standard pagination response.
pub struct Page<T> {
	pub items:       Vec<T>,
	pub page_size:   i64,
	pub page:        i64,
	pub total:       i64,
	pub total_pages: i64,
}

impl<T> Page<T> {
	pub fn new(items: Vec<T>, pagination: Pagination, total: i64) -> Self {
		let total_pages = if pagination.page_size == 0 {
			0
		} else {
			(total + pagination.page_size - 1) / pagination.page_size
		};
		Self {
			items,
			page_size: pagination.page_size,
			page: pagination.page,
			total,
			total_pages,
		}
	}

	pub fn inner(&self) -> &Vec<T> {
		&self.items
	}
}

impl<T> From<Page<T>> for Vec<T> {
	fn from(val: Page<T>) -> Self {
		val.items
	}
}
