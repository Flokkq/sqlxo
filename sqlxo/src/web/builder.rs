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
		JoinPayload,
		WebAggregateExpression,
		WebDeleteFilter,
		WebExpression,
		WebQueryError,
		WebReadFilter,
		WebSearchPayload,
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
	FullTextSearchJoinConfig,
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

fn flatten_join_payload<J>(payload: &[JoinPayload<J>]) -> Vec<Vec<String>>
where
	J: WebJoinPayload,
{
	let mut flattened_all = Vec::new();
	for node in payload {
		let mut prefix = Vec::new();
		let mut flattened = Vec::new();
		node.inner().flatten(&mut prefix, &mut flattened);
		flattened_all.extend(flattened);
	}
	flattened_all
}

fn format_join_path(path: &[String]) -> String {
	path.join(".")
}

fn resolve_search_join<C>(
	segments: &[String],
) -> Result<<C::Model as FullTextSearchable>::FullTextSearchJoin, WebQueryError>
where
	C: QueryContext,
	C::Model: FullTextSearchable,
{
	let refs: Vec<&str> = segments.iter().map(String::as_str).collect();
	<C::Model as FullTextSearchable>::resolve_search_join_path(&refs)
		.ok_or_else(|| WebQueryError::SearchJoinInvalid {
			model: std::any::type_name::<C::Model>(),
			path:  format_join_path(segments),
		})
}

#[derive(Clone)]
struct ParsedWebSearch {
	query:           String,
	language:        Option<String>,
	include_rank:    Option<bool>,
	fuzzy:           Option<bool>,
	fuzzy_threshold: Option<f64>,
	join_paths:      Vec<Vec<String>>,
}

impl ParsedWebSearch {
	fn new<D>(payload: &WebSearchPayload<D>) -> Self
	where
		D: WebQueryModel,
	{
		let join_paths = payload
			.joins
			.as_ref()
			.map(|nodes| flatten_join_payload(nodes))
			.unwrap_or_default();
		Self {
			query: payload.query.clone(),
			language: payload.language.clone(),
			include_rank: payload.include_rank,
			fuzzy: payload.fuzzy,
			fuzzy_threshold: payload.fuzzy_threshold,
			join_paths,
		}
	}

	fn ensure_join_paths(
		&self,
		available: Option<&[Vec<String>]>,
	) -> Result<(), WebQueryError> {
		if self.join_paths.is_empty() {
			return Ok(());
		}

		let Some(existing) = available else {
			let path = format_join_path(&self.join_paths[0]);
			return Err(WebQueryError::SearchJoinNotLoaded { path });
		};

		for requested in &self.join_paths {
			if !existing.iter().any(|path| path == requested) {
				return Err(WebQueryError::SearchJoinNotLoaded {
					path: format_join_path(requested),
				});
			}
		}
		Ok(())
	}

	fn join_paths(&self) -> &[Vec<String>] {
		&self.join_paths
	}
}

trait SearchApplier<'a, C: QueryContext> {
	fn apply(
		builder: ReadQueryBuilder<'a, C>,
		search: &ParsedWebSearch,
	) -> Result<ReadQueryBuilder<'a, C>, WebQueryError>;
}

struct SearchBridge<C: QueryContext>(std::marker::PhantomData<C>);

impl<'a, C> SearchApplier<'a, C> for SearchBridge<C>
where
	C: QueryContext,
{
	default fn apply(
		_builder: ReadQueryBuilder<'a, C>,
		_search: &ParsedWebSearch,
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
		FullTextSearchConfigBuilder
			+ FullTextSearchJoinConfig<
				Join = <C::Model as FullTextSearchable>::FullTextSearchJoin,
			> + Send
			+ Sync
			+ 'static,
{
	fn apply(
		builder: ReadQueryBuilder<'a, C>,
		search: &ParsedWebSearch,
	) -> Result<ReadQueryBuilder<'a, C>, WebQueryError> {
		let mut config =
			<<C::Model as FullTextSearchable>::FullTextSearchConfig as
				FullTextSearchConfigBuilder>::new_with_query(
				search.query.clone(),
			)
			.apply_language(search.language.clone())
			.apply_rank(search.include_rank)
			.apply_fuzzy(search.fuzzy)
			.apply_fuzzy_threshold(search.fuzzy_threshold);
		for join in search.join_paths() {
			let resolved = resolve_search_join::<C>(join)?;
			config =
				<<C::Model as FullTextSearchable>::FullTextSearchConfig as
					FullTextSearchJoinConfig>::with_join(config, resolved);
		}
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
	joins:         Option<Vec<JoinPath>>,
	join_segments: Option<Vec<Vec<String>>>,
	filter_expr:   Option<Expression<C::Query>>,
	sort_expr:     Option<SortOrder<C::Sort>>,
	pagination:    Option<Pagination>,
	search:        Option<ParsedWebSearch>,
	having:        Option<Vec<crate::select::HavingPredicate>>,
	_marker:       std::marker::PhantomData<D>,
}

impl<C, D> ParsedWebReadQuery<C, D>
where
	C: QueryContext,
	D: WebQueryModel + Bind<C> + AggregateBindable<C>,
{
	fn new(filter: &WebReadFilter<D>) -> Self {
		let (joins, join_segments) = if let Some(nodes) = filter.joins.as_ref()
		{
			let flattened = flatten_join_payload(nodes);
			let resolved = flattened
				.iter()
				.map(|segments| resolve_web_join::<C>(segments))
				.collect();
			(Some(resolved), Some(flattened))
		} else {
			(None, None)
		};

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
		let search = filter.search.as_ref().map(ParsedWebSearch::new::<D>);
		let having = filter.having.as_ref().map(|expr| {
			let mut preds = Vec::new();
			collect_having_predicates::<C, D>(expr, &mut preds);
			preds
		});

		Self {
			joins,
			join_segments,
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
			join_segments,
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
			search.ensure_join_paths(join_segments.as_deref())?;
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
