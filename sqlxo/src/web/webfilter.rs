use crate::{
	builder::QueryBuilder,
	expression::Expression,
	pagination::Pagination,
	sort::SortOrder,
	DtoExpression,
	DtoFilter,
	GenericDtoExpression,
};
use sqlxo_traits::{
	Bind,
	QueryContext,
	WebQueryModel,
};

fn map_expr<C, D>(e: &DtoExpression<D>) -> Expression<C::Query>
where
	C: QueryContext,
	D: WebQueryModel + Bind<C>,
{
	use GenericDtoExpression as E;
	match e {
		E::And { and } => {
			Expression::And(and.iter().map(map_expr::<C, D>).collect())
		}
		E::Or { or } => {
			Expression::Or(or.iter().map(map_expr::<C, D>).collect())
		}
		E::Leaf(l) => Expression::Leaf(<D as Bind<C>>::map_leaf(l)),
	}
}

impl<'a, C> QueryBuilder<'a, C>
where
	C: QueryContext,
{
	pub fn from_dto<D>(dto: &DtoFilter<D>) -> Self
	where
		D: WebQueryModel + Bind<C>,
	{
		let mut qb = QueryBuilder::<C>::from_ctx();

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
				page:      p.page_no as i64,
				page_size: p.page_size as i64,
			});
		}

		qb
	}
}
