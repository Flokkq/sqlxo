use crate::{
	blocks::{
		BuildableFilter,
		BuildableJoin,
		BuildablePage,
		BuildableSort,
		Expression,
		Pagination,
		SortOrder,
	},
	select::HavingList,
	web::{
		AggregateBindable,
		GenericWebExpression,
		WebAggregateExpression,
		WebExpression,
		WebFilter,
		WebJoinKind,
		WebJoinPath,
		WebSearch,
	},
	QueryBuilder,
	ReadQueryBuilder,
};
use sqlxo_traits::{
	Bind,
	FullTextSearchConfigBuilder,
	FullTextSearchable,
	JoinKind,
	JoinPath,
	QueryContext,
	WebJoinGraph,
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

fn collect_having_predicates<C, D>(
	expr: &WebAggregateExpression<D>,
	out: &mut Vec<crate::select::HavingPredicate>,
) where
	C: QueryContext,
	D: WebQueryModel + AggregateBindable<C>,
{
	match expr {
		GenericWebExpression::And { and } => {
			for inner in and {
				collect_having_predicates::<C, D>(inner, out);
			}
		}
		GenericWebExpression::Or { .. } => {
			panic!("OR is not supported in aggregate filters");
		}
		GenericWebExpression::Leaf(leaf) => {
			out.push(<D as AggregateBindable<C>>::map_aggregate_leaf(leaf));
		}
	}
}

fn join_kind_from(kind: WebJoinKind) -> JoinKind {
	match kind {
		WebJoinKind::Inner => JoinKind::Inner,
		WebJoinKind::Left => JoinKind::Left,
	}
}

fn resolve_web_join<C>(join: &WebJoinPath) -> JoinPath
where
	C: QueryContext,
	C::Model: WebJoinGraph,
{
	let segments: Vec<&str> = join.path.iter().map(String::as_str).collect();
	let kind = join_kind_from(join.kind);
	<C::Model as WebJoinGraph>::resolve_join_path(&segments, kind)
		.unwrap_or_else(|| {
			panic!(
				"invalid join path {:?} for model {}",
				join.path,
				std::any::type_name::<C::Model>()
			);
		})
}

trait SearchApplier<'a, C: QueryContext> {
	fn apply(
		builder: ReadQueryBuilder<'a, C>,
		search: &WebSearch,
	) -> ReadQueryBuilder<'a, C>;
}

struct SearchBridge<C: QueryContext>(std::marker::PhantomData<C>);

impl<'a, C> SearchApplier<'a, C> for SearchBridge<C>
where
	C: QueryContext,
{
	default fn apply(
		_builder: ReadQueryBuilder<'a, C>,
		_search: &WebSearch,
	) -> ReadQueryBuilder<'a, C> {
		panic!(
			"full-text search is not supported for {}",
			std::any::type_name::<C::Model>()
		);
	}
}

impl<'a, C> SearchApplier<'a, C> for SearchBridge<C>
where
	C: QueryContext,
	C::Model: FullTextSearchable + 'static,
	<C::Model as FullTextSearchable>::FullTextSearchConfig:
		FullTextSearchConfigBuilder + Send + Sync + 'static,
{
	fn apply(
		builder: ReadQueryBuilder<'a, C>,
		search: &WebSearch,
	) -> ReadQueryBuilder<'a, C> {
		let config =
			<<C::Model as FullTextSearchable>::FullTextSearchConfig as
				FullTextSearchConfigBuilder>::new_with_query(
				search.query.clone(),
			)
			.apply_language(search.language.clone())
			.apply_rank(search.include_rank);
		builder.search(config)
	}
}

impl<'a, C> QueryBuilder<C>
where
	C: QueryContext,
{
	pub fn from_dto<D>(dto: &WebFilter<D>) -> ReadQueryBuilder<'a, C>
	where
		D: WebQueryModel + Bind<C> + AggregateBindable<C>,
	{
		let mut qb = QueryBuilder::<C>::read();

		if let Some(joins) = dto.joins.as_ref() {
			for join in joins {
				let path = resolve_web_join::<C>(join);
				qb = qb.join_path(path);
			}
		}

		if let Some(f) = &dto.filter {
			let expr = map_expr::<C, D>(f);
			qb = qb.r#where(expr);
		}

		if let Some(search) = &dto.search {
			qb = <SearchBridge<C> as SearchApplier<'a, C>>::apply(qb, search);
		}

		if let Some(having_expr) = &dto.having {
			let mut preds = Vec::new();
			collect_having_predicates::<C, D>(having_expr, &mut preds);
			if !preds.is_empty() {
				qb = qb.having(HavingList::new(preds));
			}
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
