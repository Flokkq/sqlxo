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
	DeleteQueryBuilder,
	QueryBuilder,
	ReadQueryBuilder,
	UpdateQueryBuilder,
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
	pub fn from_web_query<D>(dto: &WebFilter<D>) -> WebQueryAdapter<'a, C, D>
	where
		D: WebQueryModel + Bind<C> + AggregateBindable<C>,
	{
		let parsed = ParsedWebQuery::new(dto);
		WebQueryAdapter {
			parsed,
			_marker: std::marker::PhantomData,
		}
	}
}

pub struct WebQueryAdapter<'a, C, D>
where
	C: QueryContext,
	D: WebQueryModel + Bind<C> + AggregateBindable<C>,
{
	parsed:  ParsedWebQuery<C, D>,
	_marker: std::marker::PhantomData<&'a ()>,
}

impl<'a, C, D> WebQueryAdapter<'a, C, D>
where
	C: QueryContext,
	D: WebQueryModel + Bind<C> + AggregateBindable<C>,
{
	pub fn into_read(self) -> ReadQueryBuilder<'a, C>
	where
		C::Model: crate::GetDeleteMarker + sqlxo_traits::JoinNavigationModel,
	{
		self.parsed.into_read_builder()
	}

	pub fn into_update(self) -> UpdateQueryBuilder<'a, C>
	where
		C::Model: crate::Updatable,
	{
		self.parsed.into_update_builder()
	}

	pub fn into_delete(self) -> DeleteQueryBuilder<'a, C>
	where
		C::Model: crate::Deletable,
	{
		self.parsed.into_delete_builder()
	}
}

#[derive(Clone)]
struct ParsedWebQuery<C, D>
where
	C: QueryContext,
	D: WebQueryModel + Bind<C> + AggregateBindable<C>,
{
	joins:       Option<Vec<JoinPath>>,
	filter_expr: Option<Expression<C::Query>>,
	sort_expr:   Option<SortOrder<C::Sort>>,
	pagination:  Option<Pagination>,
	search:      Option<WebSearch>,
	having:      Option<Vec<crate::select::HavingPredicate>>,
	_marker:     std::marker::PhantomData<D>,
}

impl<C, D> ParsedWebQuery<C, D>
where
	C: QueryContext,
	D: WebQueryModel + Bind<C> + AggregateBindable<C>,
{
	fn new(filter: &WebFilter<D>) -> Self {
		let joins = filter.joins.as_ref().map(|paths| {
			paths
				.iter()
				.map(|path| resolve_web_join::<C>(path))
				.collect()
		});

		let filter_expr = filter.filter.as_ref().map(map_expr::<C, D>);

		let sort_expr = filter
			.sort
			.as_ref()
			.and_then(|sorts| if sorts.is_empty() { None } else { Some(sorts) })
			.map(|sorts| {
				let entries: Vec<C::Sort> = sorts
					.iter()
					.map(|s| <D as Bind<C>>::map_sort_field(&s.0))
					.collect();
				SortOrder::from(entries)
			});

		let pagination = filter.page.map(|p| Pagination {
			page:      p.page,
			page_size: p.page_size,
		});
		let search = filter.search.clone();
		let having = filter.having.as_ref().map(|expr| {
			let mut preds = Vec::new();
			collect_having_predicates::<C, D>(expr, &mut preds);
			preds
		});

		Self {
			joins,
			filter_expr,
			sort_expr,
			pagination,
			search,
			having,
			_marker: std::marker::PhantomData,
		}
	}

	fn into_read_builder<'a>(self) -> ReadQueryBuilder<'a, C>
	where
		C::Model: crate::GetDeleteMarker + sqlxo_traits::JoinNavigationModel,
	{
		let ParsedWebQuery {
			joins,
			filter_expr,
			sort_expr,
			pagination,
			search,
			having,
			..
		} = self;

		let mut builder = QueryBuilder::<C>::read();

		if let Some(joins) = joins {
			for path in joins {
				builder = builder.join_path(path);
			}
		}

		if let Some(expr) = filter_expr {
			builder = builder.r#where(expr);
		}

		if let Some(search) = search {
			builder = <SearchBridge<C> as SearchApplier<'a, C>>::apply(
				builder, &search,
			);
		}

		if let Some(preds) = having {
			if !preds.is_empty() {
				builder = builder.having(HavingList::new(preds));
			}
		}

		if let Some(sort) = sort_expr {
			builder = builder.order_by(sort);
		}

		if let Some(page) = pagination {
			builder = builder.paginate(page);
		}

		builder
	}

	fn into_update_builder<'a>(self) -> UpdateQueryBuilder<'a, C>
	where
		C::Model: crate::Updatable,
	{
		let ParsedWebQuery {
			filter_expr,
			joins,
			search,
			having,
			..
		} = self;
		Self::assert_mutation_support(
			"update",
			joins.is_some(),
			search.is_some(),
			has_having(&having),
		);
		let mut builder = QueryBuilder::<C>::update();
		if let Some(expr) = filter_expr {
			builder = builder.r#where(expr);
		}
		builder
	}

	fn into_delete_builder<'a>(self) -> DeleteQueryBuilder<'a, C>
	where
		C::Model: crate::Deletable,
	{
		let ParsedWebQuery {
			filter_expr,
			joins,
			search,
			having,
			..
		} = self;
		Self::assert_mutation_support(
			"delete",
			joins.is_some(),
			search.is_some(),
			has_having(&having),
		);
		let mut builder = QueryBuilder::<C>::delete();
		if let Some(expr) = filter_expr {
			builder = builder.r#where(expr);
		}
		builder
	}

	fn assert_mutation_support(
		op: &str,
		has_joins: bool,
		has_search: bool,
		has_having: bool,
	) {
		if has_joins {
			panic!(
				"webquery joins are only supported for read operations \
				 (attempted {})",
				op
			);
		}
		if has_search {
			panic!(
				"full-text search filters are only supported for read \
				 operations (attempted {})",
				op
			);
		}
		if has_having {
			panic!(
				"HAVING/aggregate filters are only supported for read \
				 operations (attempted {})",
				op
			);
		}
	}
}

fn has_having(having: &Option<Vec<crate::select::HavingPredicate>>) -> bool {
	having
		.as_ref()
		.map(|preds| !preds.is_empty())
		.unwrap_or(false)
}
