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
		WebDeleteFilter,
		WebExpression,
		WebQueryError,
		WebReadFilter,
		WebSearch,
		WebUpdateFilter,
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
	WebJoinPayload,
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

fn resolve_web_join<C>(segments: &[String]) -> JoinPath
where
	C: QueryContext,
	C::Model: WebJoinGraph,
{
	let refs: Vec<&str> = segments.iter().map(String::as_str).collect();
	<C::Model as WebJoinGraph>::resolve_join_path(&refs, JoinKind::Left)
		.unwrap_or_else(|| {
			panic!(
				"invalid join path {:?} for model {}",
				segments,
				std::any::type_name::<C::Model>()
			);
		})
}

trait SearchApplier<'a, C: QueryContext> {
	fn apply(
		builder: ReadQueryBuilder<'a, C>,
		search: &WebSearch,
	) -> Result<ReadQueryBuilder<'a, C>, WebQueryError>;
}

struct SearchBridge<C: QueryContext>(std::marker::PhantomData<C>);

impl<'a, C> SearchApplier<'a, C> for SearchBridge<C>
where
	C: QueryContext,
{
	default fn apply(
		_builder: ReadQueryBuilder<'a, C>,
		_search: &WebSearch,
	) -> Result<ReadQueryBuilder<'a, C>, WebQueryError> {
		Err(WebQueryError::SearchUnsupported {
			model: std::any::type_name::<C::Model>(),
		})
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
	) -> Result<ReadQueryBuilder<'a, C>, WebQueryError> {
		let config =
			<<C::Model as FullTextSearchable>::FullTextSearchConfig as
				FullTextSearchConfigBuilder>::new_with_query(
				search.query.clone(),
			)
			.apply_language(search.language.clone())
			.apply_rank(search.include_rank)
			.apply_fuzzy(search.fuzzy)
			.apply_fuzzy_threshold(search.fuzzy_threshold);
		Ok(builder.search(config))
	}
}

impl<'a, C> QueryBuilder<C>
where
	C: QueryContext,
{
	pub fn try_from_web_read<D>(
		dto: &WebReadFilter<D>,
	) -> Result<ReadQueryBuilder<'a, C>, WebQueryError>
	where
		D: WebQueryModel + Bind<C> + AggregateBindable<C>,
		C::Model: crate::GetDeleteMarker + sqlxo_traits::JoinNavigationModel,
	{
		ParsedWebReadQuery::<C, D>::new(dto).into_read_builder()
	}

	pub fn from_web_read<D>(dto: &WebReadFilter<D>) -> ReadQueryBuilder<'a, C>
	where
		D: WebQueryModel + Bind<C> + AggregateBindable<C>,
		C::Model: crate::GetDeleteMarker + sqlxo_traits::JoinNavigationModel,
	{
		Self::try_from_web_read::<D>(dto).expect(
			"use `QueryBuilder::try_from_web_read` to handle unsupported \
			 search payloads or other web query validation errors",
		)
	}

	pub fn from_web_update<D>(
		dto: &WebUpdateFilter<D>,
	) -> UpdateQueryBuilder<'a, C>
	where
		D: WebQueryModel + Bind<C>,
		C::Model: crate::Updatable,
	{
		apply_mutation_filter::<C, D, UpdateQueryBuilder<'a, C>>(
			QueryBuilder::<C>::update(),
			dto.filter.as_ref(),
		)
	}

	pub fn from_web_delete<D>(
		dto: &WebDeleteFilter<D>,
	) -> DeleteQueryBuilder<'a, C>
	where
		D: WebQueryModel + Bind<C>,
		C::Model: crate::Deletable,
	{
		apply_mutation_filter::<C, D, DeleteQueryBuilder<'a, C>>(
			QueryBuilder::<C>::delete(),
			dto.filter.as_ref(),
		)
	}
}

fn apply_mutation_filter<C, D, B>(
	mut builder: B,
	filter: Option<&WebExpression<D>>,
) -> B
where
	C: QueryContext,
	D: WebQueryModel + Bind<C>,
	B: BuildableFilter<C>,
{
	if let Some(expr) = filter.map(map_expr::<C, D>) {
		builder = builder.r#where(expr);
	}
	builder
}

#[derive(Clone)]
struct ParsedWebReadQuery<C, D>
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

impl<C, D> ParsedWebReadQuery<C, D>
where
	C: QueryContext,
	D: WebQueryModel + Bind<C> + AggregateBindable<C>,
{
	fn new(filter: &WebReadFilter<D>) -> Self {
		let joins = filter.joins.as_ref().map(|nodes| {
			let mut resolved = Vec::new();
			for node in nodes {
				let mut prefix = Vec::new();
				let mut flattened: Vec<Vec<String>> = Vec::new();
				node.inner().flatten(&mut prefix, &mut flattened);
				for segments in flattened {
					resolved.push(resolve_web_join::<C>(&segments));
				}
			}
			resolved
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

	fn into_read_builder<'a>(
		self,
	) -> Result<ReadQueryBuilder<'a, C>, WebQueryError>
	where
		C::Model: crate::GetDeleteMarker + sqlxo_traits::JoinNavigationModel,
	{
		let ParsedWebReadQuery {
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
			builder = SearchBridge::<C>::apply(builder, &search)?;
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

		Ok(builder)
	}
}
