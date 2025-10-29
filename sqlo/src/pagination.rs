#[derive(Debug, Clone, Copy)]
pub struct Pagination {
    pub page: i64,
    pub page_size: i64,
}

impl Pagination {
    pub fn all() -> Self {
        Self {
            page: 0,
            page_size: i32::MAX as i64,
        }
    }
}

impl Default for Pagination {
    fn default() -> Self {
        Pagination::all()
    }
}
