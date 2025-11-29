use crate::{
	blocks::{
		BuildableFilter,
		BuildablePage,
		BuildableSort,
		Expression,
		Pagination,
		SortOrder,
	},
	web::{
		GenericWebExpression,
		WebExpression,
		WebFilter,
	},
	QueryBuilder,
	ReadQueryBuilder,
};
use sqlxo_traits::{
	Bind,
	QueryContext,
	WebQueryModel,
};

fn map_expr<C, D>(e: &WebExpression<D>) -> Expression<C::Query>
where
	C: QueryContext,
	D: WebQueryModel + Bind<C>,
{
	match e {
		GenericWebExpression::And { and } => {
			Expression::And(and.iter().map(map_expr::<C, D>).collect())
		}
		GenericWebExpression::Or { or } => {
			Expression::Or(or.iter().map(map_expr::<C, D>).collect())
		}
		GenericWebExpression::Leaf(l) => {
			Expression::Leaf(<D as Bind<C>>::map_leaf(l))
		}
	}
}

impl<'a, C> QueryBuilder<C>
where
	C: QueryContext,
{
	pub fn from_dto<D>(dto: &WebFilter<D>) -> ReadQueryBuilder<'a, C>
	where
		D: WebQueryModel + Bind<C>,
	{
		let mut qb = QueryBuilder::<C>::insert();

		if let Some(f) = &dto.filter {
			let expr = map_expr::<C, D>(f);
			qb = qb.r#where(expr);
		}

		if let Some(sorts_in) = dto.sort.as_ref().filter(|s| !s.is_empty()) {
			let sorts: Vec<C::Sort> = sorts_in
				.iter()
				.map(|s| <D as Bind<C>>::map_sort_field(&s.0))
				.collect();
			qb = qb.order_by(SortOrder::from(sorts));
		}

		if let Some(p) = dto.page {
			qb = qb.paginate(Pagination {
				page:      p.page,
				page_size: p.page_size,
			});
		}

		qb
	}
}
