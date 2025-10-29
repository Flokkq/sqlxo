use sqlo_traits::SqlWrite;
use sqlo_traits::{Filterable, Sortable, SqlJoin};
use sqlx::{Postgres, Type};

use crate::expression::Expression;
use crate::head::SqlHead;
use crate::pagination::Pagination;
use crate::sort::SortOrder;

pub struct SqlWriter {
    qb: sqlx::QueryBuilder<'static, Postgres>,
    has_join: bool,
    has_where: bool,
    has_sort: bool,
    has_pagination: bool,
}

impl SqlWriter {
    pub fn new(head: SqlHead) -> Self {
        let qb = sqlx::QueryBuilder::<Postgres>::new(head.to_string());

        Self {
            qb,
            has_join: false,
            has_where: false,
            has_sort: false,
            has_pagination: false,
        }
    }

    pub fn into_builder(self) -> sqlx::QueryBuilder<'static, Postgres> {
        self.qb
    }

    pub fn push_joins<J: SqlJoin>(&mut self, joins: &Vec<J>) {
        if self.has_join {
            return;
        }

        for j in joins {
            self.qb.push(j.to_sql());
        }
    }

    pub fn push_where<F: Filterable>(&mut self, expr: &Expression<F>) {
        if self.has_where {
            return;
        }

        self.qb.push(" WHERE ");
        self.has_where = true;
        expr.write(self);
    }

    pub fn push_sort<S: Sortable>(&mut self, sort: &SortOrder<S>) {
        if self.has_sort {
            return;
        }

        self.qb.push(" ORDER BY ");
        self.has_sort = true;
        self.qb.push(sort.to_sql());
    }

    pub fn push_pagination(&mut self, p: &Pagination) {
        if self.has_pagination {
            return;
        }

        self.qb.push(" LIMIT ");
        self.bind(p.page_size);
        self.qb.push(" OFFSET ");
        self.bind(p.page * p.page_size);
    }
}

impl SqlWrite for SqlWriter {
    fn push(&mut self, s: &str) {
        self.qb.push(s);
    }

    fn bind<T>(&mut self, value: T)
    where
        T: sqlx::Encode<'static, Postgres> + Send + 'static,
        T: Type<Postgres>,
    {
        self.qb.push_bind(value);
    }
}
