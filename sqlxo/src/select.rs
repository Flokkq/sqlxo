use smallvec::SmallVec;
use sqlx::{
	Encode,
	Postgres,
	QueryBuilder,
	Type,
};
use sqlxo_traits::{
	QueryModel,
	SqlWrite,
};
use std::{
	marker::PhantomData,
	sync::Arc,
};

use crate::blocks::SqlWriter;

/// Marker trait for model columns that can participate in `take!`.
pub trait Column: Copy {
	type Model: QueryModel;
	type Type;
	const NAME: &'static str;
	const TABLE: &'static str;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectionColumn {
	pub table:  &'static str,
	pub column: &'static str,
}

impl SelectionColumn {
	pub const fn new(table: &'static str, column: &'static str) -> Self {
		Self { table, column }
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AggregateFunction {
	Count,
	CountDistinct,
	Sum,
	Avg,
	Min,
	Max,
}

impl AggregateFunction {
	pub const fn sql_name(&self) -> &'static str {
		match self {
			Self::Count | Self::CountDistinct => "COUNT",
			Self::Sum => "SUM",
			Self::Avg => "AVG",
			Self::Min => "MIN",
			Self::Max => "MAX",
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AggregateSelection {
	pub function: AggregateFunction,
	pub column:   Option<SelectionColumn>,
}

impl AggregateSelection {
	pub const fn new(
		function: AggregateFunction,
		column: Option<SelectionColumn>,
	) -> Self {
		Self { function, column }
	}

	pub const fn with_column(
		function: AggregateFunction,
		column: SelectionColumn,
	) -> Self {
		Self {
			function,
			column: Some(column),
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectionEntry {
	Column(SelectionColumn),
	Aggregate(AggregateSelection),
}

#[derive(Debug, Clone)]
pub struct SelectionList<Output, Store = SelectionColumn> {
	pub(crate) entries: SmallVec<[Store; 4]>,
	_marker:            PhantomData<Output>,
}

impl<Output, Store> SelectionList<Output, Store> {
	pub fn new(entries: SmallVec<[Store; 4]>) -> Self {
		Self {
			entries,
			_marker: PhantomData,
		}
	}

	pub fn entries(&self) -> &[Store] {
		&self.entries
	}

	pub fn clone_entries(&self) -> SmallVec<[Store; 4]>
	where
		Store: Clone,
	{
		self.entries.clone()
	}

	pub fn len(&self) -> usize {
		self.entries.len()
	}

	pub fn is_empty(&self) -> bool {
		self.entries.is_empty()
	}
}

impl<Output> SelectionList<Output, SelectionColumn> {
	pub fn columns(&self) -> &[SelectionColumn] {
		&self.entries
	}

	pub fn clone_columns(&self) -> SmallVec<[SelectionColumn; 4]> {
		self.entries.clone()
	}

	pub fn push_returning(
		&self,
		qb: &mut QueryBuilder<'static, Postgres>,
		table: &str,
	) {
		qb.push(" RETURNING ");
		for (idx, col) in self.entries.iter().enumerate() {
			assert_eq!(
				col.table, table,
				"`RETURNING` may only use columns from `{}` but got `{}`",
				table, col.table,
			);

			if idx > 0 {
				qb.push(", ");
			}
			qb.push(&format!(r#""{}"."{}""#, table, col.column));
		}
	}
}

impl<Output> SelectionList<Output, SelectionEntry> {
	pub fn expect_columns(self) -> SelectionList<Output, SelectionColumn> {
		let mut cols = SmallVec::<[SelectionColumn; 4]>::new();
		for entry in self.entries {
			match entry {
				SelectionEntry::Column(col) => cols.push(col),
				SelectionEntry::Aggregate(_) => {
					panic!("aggregates are not supported in this context")
				}
			}
		}
		SelectionList::new(cols)
	}
}

pub fn push_returning<Output>(
	qb: &mut QueryBuilder<'static, Postgres>,
	table: &str,
	selection: Option<&SelectionList<Output, SelectionColumn>>,
) {
	if let Some(sel) = selection {
		sel.push_returning(qb, table);
	} else {
		qb.push(" RETURNING *");
	}
}

#[derive(Debug, Clone)]
pub struct GroupByList {
	columns: SmallVec<[SelectionColumn; 4]>,
}

impl GroupByList {
	pub fn new(columns: SmallVec<[SelectionColumn; 4]>) -> Self {
		Self { columns }
	}

	pub fn columns(&self) -> &[SelectionColumn] {
		&self.columns
	}

	pub fn into_columns(self) -> SmallVec<[SelectionColumn; 4]> {
		self.columns
	}
}

#[derive(Clone)]
pub struct HavingValue {
	binder: Arc<dyn Fn(&mut SqlWriter) + Send + Sync>,
}

impl HavingValue {
	pub fn new<T>(value: T) -> Self
	where
		T: Clone + Send + Sync + 'static,
		T: Encode<'static, Postgres>,
		T: Type<Postgres>,
	{
		let value = Arc::new(value);
		Self {
			binder: Arc::new(move |writer: &mut SqlWriter| {
				writer.bind((value.as_ref()).clone());
			}),
		}
	}

	pub fn bind(&self, writer: &mut SqlWriter) {
		(self.binder)(writer);
	}
}

#[derive(Clone, Copy, Debug)]
pub enum ComparisonOp {
	Eq,
	Ne,
	Gt,
	Ge,
	Lt,
	Le,
}

impl ComparisonOp {
	pub const fn as_str(&self) -> &'static str {
		match self {
			Self::Eq => "=",
			Self::Ne => "!=",
			Self::Gt => ">",
			Self::Ge => ">=",
			Self::Lt => "<",
			Self::Le => "<=",
		}
	}
}

#[derive(Clone)]
pub struct HavingPredicate {
	pub selection:  AggregateSelection,
	pub comparator: ComparisonOp,
	value:          HavingValue,
}

impl HavingPredicate {
	pub fn new(
		selection: AggregateSelection,
		comparator: ComparisonOp,
		value: HavingValue,
	) -> Self {
		Self {
			selection,
			comparator,
			value,
		}
	}

	pub fn bind_value(&self, writer: &mut SqlWriter) {
		self.value.bind(writer);
	}
}

#[derive(Clone)]
pub struct HavingList {
	predicates: Vec<HavingPredicate>,
}

impl HavingList {
	pub fn new(predicates: Vec<HavingPredicate>) -> Self {
		Self { predicates }
	}

	pub fn predicates(&self) -> &[HavingPredicate] {
		&self.predicates
	}

	pub fn into_predicates(self) -> Vec<HavingPredicate> {
		self.predicates
	}
}

pub trait SelectionExpr {
	type Output;
	fn record(self, entries: &mut SmallVec<[SelectionEntry; 4]>);
}

impl<T> SelectionExpr for T
where
	T: Column,
{
	type Output = T::Type;

	fn record(self, entries: &mut SmallVec<[SelectionEntry; 4]>) {
		let column = SelectionColumn::new(T::TABLE, T::NAME);
		entries.push(SelectionEntry::Column(column));
	}
}

#[derive(Clone, Copy)]
pub struct CountAllExpr;

impl CountAllExpr {
	pub const fn new() -> Self {
		Self
	}
}

#[derive(Clone, Copy)]
pub struct CountExpr<C: Column>(PhantomData<C>);

impl<C: Column> CountExpr<C> {
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

#[derive(Clone, Copy)]
pub struct CountDistinctExpr<C: Column>(PhantomData<C>);

impl<C: Column> CountDistinctExpr<C> {
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

#[derive(Clone, Copy)]
pub struct SumExpr<C: Column>(PhantomData<C>);

impl<C: Column> SumExpr<C> {
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

#[derive(Clone, Copy)]
pub struct AvgExpr<C: Column>(PhantomData<C>);

impl<C: Column> AvgExpr<C> {
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

#[derive(Clone, Copy)]
pub struct MinExpr<C: Column>(PhantomData<C>);

impl<C: Column> MinExpr<C> {
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

#[derive(Clone, Copy)]
pub struct MaxExpr<C: Column>(PhantomData<C>);

impl<C: Column> MaxExpr<C> {
	pub const fn new() -> Self {
		Self(PhantomData)
	}
}

impl SelectionExpr for CountAllExpr {
	type Output = i64;

	fn record(self, entries: &mut SmallVec<[SelectionEntry; 4]>) {
		entries.push(SelectionEntry::Aggregate(self.selection()));
	}
}

impl AggregateSelectionExpr for CountAllExpr {
	fn selection(&self) -> AggregateSelection {
		AggregateSelection::new(AggregateFunction::Count, None)
	}
}

impl<C> SelectionExpr for CountExpr<C>
where
	C: Column,
{
	type Output = i64;

	fn record(self, entries: &mut SmallVec<[SelectionEntry; 4]>) {
		entries.push(SelectionEntry::Aggregate(self.selection()));
	}
}

impl<C> AggregateSelectionExpr for CountExpr<C>
where
	C: Column,
{
	fn selection(&self) -> AggregateSelection {
		let column = SelectionColumn::new(C::TABLE, C::NAME);
		AggregateSelection::with_column(AggregateFunction::Count, column)
	}
}

impl<C> SelectionExpr for CountDistinctExpr<C>
where
	C: Column,
{
	type Output = i64;

	fn record(self, entries: &mut SmallVec<[SelectionEntry; 4]>) {
		entries.push(SelectionEntry::Aggregate(self.selection()));
	}
}

impl<C> AggregateSelectionExpr for CountDistinctExpr<C>
where
	C: Column,
{
	fn selection(&self) -> AggregateSelection {
		let column = SelectionColumn::new(C::TABLE, C::NAME);
		AggregateSelection::with_column(
			AggregateFunction::CountDistinct,
			column,
		)
	}
}

impl<C> SelectionExpr for SumExpr<C>
where
	C: Column,
{
	type Output = Option<C::Type>;

	fn record(self, entries: &mut SmallVec<[SelectionEntry; 4]>) {
		entries.push(SelectionEntry::Aggregate(self.selection()));
	}
}

impl<C> AggregateSelectionExpr for SumExpr<C>
where
	C: Column,
{
	fn selection(&self) -> AggregateSelection {
		let column = SelectionColumn::new(C::TABLE, C::NAME);
		AggregateSelection::with_column(AggregateFunction::Sum, column)
	}
}

impl<C> SelectionExpr for AvgExpr<C>
where
	C: Column,
{
	type Output = Option<C::Type>;

	fn record(self, entries: &mut SmallVec<[SelectionEntry; 4]>) {
		entries.push(SelectionEntry::Aggregate(self.selection()));
	}
}

impl<C> AggregateSelectionExpr for AvgExpr<C>
where
	C: Column,
{
	fn selection(&self) -> AggregateSelection {
		let column = SelectionColumn::new(C::TABLE, C::NAME);
		AggregateSelection::with_column(AggregateFunction::Avg, column)
	}
}

impl<C> SelectionExpr for MinExpr<C>
where
	C: Column,
{
	type Output = Option<C::Type>;

	fn record(self, entries: &mut SmallVec<[SelectionEntry; 4]>) {
		entries.push(SelectionEntry::Aggregate(self.selection()));
	}
}

impl<C> AggregateSelectionExpr for MinExpr<C>
where
	C: Column,
{
	fn selection(&self) -> AggregateSelection {
		let column = SelectionColumn::new(C::TABLE, C::NAME);
		AggregateSelection::with_column(AggregateFunction::Min, column)
	}
}

impl<C> SelectionExpr for MaxExpr<C>
where
	C: Column,
{
	type Output = Option<C::Type>;

	fn record(self, entries: &mut SmallVec<[SelectionEntry; 4]>) {
		entries.push(SelectionEntry::Aggregate(self.selection()));
	}
}

impl<C> AggregateSelectionExpr for MaxExpr<C>
where
	C: Column,
{
	fn selection(&self) -> AggregateSelection {
		let column = SelectionColumn::new(C::TABLE, C::NAME);
		AggregateSelection::with_column(AggregateFunction::Max, column)
	}
}

pub trait AggregateSelectionExpr: Copy {
	fn selection(&self) -> AggregateSelection;
}

pub trait AggregatePredicateBuilder: AggregateSelectionExpr + Copy {
	fn compare<T>(self, op: ComparisonOp, value: T) -> HavingPredicate
	where
		T: Clone + Send + Sync + 'static,
		T: Encode<'static, Postgres>,
		T: Type<Postgres>,
	{
		HavingPredicate::new(self.selection(), op, HavingValue::new(value))
	}

	fn eq<T>(self, value: T) -> HavingPredicate
	where
		T: Clone + Send + Sync + 'static,
		T: Encode<'static, Postgres>,
		T: Type<Postgres>,
	{
		self.compare(ComparisonOp::Eq, value)
	}

	fn ne<T>(self, value: T) -> HavingPredicate
	where
		T: Clone + Send + Sync + 'static,
		T: Encode<'static, Postgres>,
		T: Type<Postgres>,
	{
		self.compare(ComparisonOp::Ne, value)
	}

	fn gt<T>(self, value: T) -> HavingPredicate
	where
		T: Clone + Send + Sync + 'static,
		T: Encode<'static, Postgres>,
		T: Type<Postgres>,
	{
		self.compare(ComparisonOp::Gt, value)
	}

	fn ge<T>(self, value: T) -> HavingPredicate
	where
		T: Clone + Send + Sync + 'static,
		T: Encode<'static, Postgres>,
		T: Type<Postgres>,
	{
		self.compare(ComparisonOp::Ge, value)
	}

	fn lt<T>(self, value: T) -> HavingPredicate
	where
		T: Clone + Send + Sync + 'static,
		T: Encode<'static, Postgres>,
		T: Type<Postgres>,
	{
		self.compare(ComparisonOp::Lt, value)
	}

	fn le<T>(self, value: T) -> HavingPredicate
	where
		T: Clone + Send + Sync + 'static,
		T: Encode<'static, Postgres>,
		T: Type<Postgres>,
	{
		self.compare(ComparisonOp::Le, value)
	}
}

impl<T> AggregatePredicateBuilder for T where T: AggregateSelectionExpr + Copy {}

macro_rules! aggregate_predicate_methods_body {
	() => {
		pub fn eq<T>(self, value: T) -> HavingPredicate
		where
			T: Clone + Send + Sync + 'static,
			T: Encode<'static, Postgres>,
			T: Type<Postgres>,
		{
			AggregatePredicateBuilder::eq(self, value)
		}

		pub fn ne<T>(self, value: T) -> HavingPredicate
		where
			T: Clone + Send + Sync + 'static,
			T: Encode<'static, Postgres>,
			T: Type<Postgres>,
		{
			AggregatePredicateBuilder::ne(self, value)
		}

		pub fn gt<T>(self, value: T) -> HavingPredicate
		where
			T: Clone + Send + Sync + 'static,
			T: Encode<'static, Postgres>,
			T: Type<Postgres>,
		{
			AggregatePredicateBuilder::gt(self, value)
		}

		pub fn ge<T>(self, value: T) -> HavingPredicate
		where
			T: Clone + Send + Sync + 'static,
			T: Encode<'static, Postgres>,
			T: Type<Postgres>,
		{
			AggregatePredicateBuilder::ge(self, value)
		}

		pub fn lt<T>(self, value: T) -> HavingPredicate
		where
			T: Clone + Send + Sync + 'static,
			T: Encode<'static, Postgres>,
			T: Type<Postgres>,
		{
			AggregatePredicateBuilder::lt(self, value)
		}

		pub fn le<T>(self, value: T) -> HavingPredicate
		where
			T: Clone + Send + Sync + 'static,
			T: Encode<'static, Postgres>,
			T: Type<Postgres>,
		{
			AggregatePredicateBuilder::le(self, value)
		}
	};
}

macro_rules! impl_aggregate_predicate_methods {
	($ty:ty) => {
		impl $ty {
			aggregate_predicate_methods_body!();
		}
	};
	($ty:ident < $gen:ident > where $($bounds:tt)+) => {
		impl<$gen> $ty<$gen>
		where
			$($bounds)+
		{
			aggregate_predicate_methods_body!();
		}
	};
}

impl_aggregate_predicate_methods!(CountAllExpr);
impl_aggregate_predicate_methods!(CountExpr<C> where C: Column);
impl_aggregate_predicate_methods!(CountDistinctExpr<C> where C: Column);
impl_aggregate_predicate_methods!(SumExpr<C> where C: Column);
impl_aggregate_predicate_methods!(AvgExpr<C> where C: Column);
impl_aggregate_predicate_methods!(MinExpr<C> where C: Column);
impl_aggregate_predicate_methods!(MaxExpr<C> where C: Column);

#[derive(Clone, Copy)]
pub struct SelectionOutput<T>(pub PhantomData<T>);

impl<T> SelectionOutput<T> {
	pub fn into_selection_list<Store>(
		self,
		entries: SmallVec<[Store; 4]>,
	) -> SelectionList<T, Store> {
		SelectionList::new(entries)
	}
}

pub fn record_selection_expr<E>(
	expr: E,
	entries: &mut SmallVec<[SelectionEntry; 4]>,
) -> SelectionOutput<E::Output>
where
	E: SelectionExpr,
{
	expr.record(entries);
	SelectionOutput(PhantomData)
}

pub trait SelectionOutputTuple {
	type Output;

	fn flatten(self) -> SelectionOutput<Self::Output>;
}

impl<A> SelectionOutputTuple for (SelectionOutput<A>,) {
	type Output = (A,);

	fn flatten(self) -> SelectionOutput<(A,)> {
		let _ = self;
		SelectionOutput(PhantomData)
	}
}

macro_rules! impl_selection_output_tuple {
	($($name:ident),+) => {
		impl<$($name),+> SelectionOutputTuple for ($(SelectionOutput<$name>,)+) {
			type Output = ($($name,)+);

			fn flatten(self) -> SelectionOutput<($($name,)+)> {
				let _ = self;
				SelectionOutput(PhantomData)
			}
		}
	};
}

impl_selection_output_tuple!(A, B);
impl_selection_output_tuple!(A, B, C);
impl_selection_output_tuple!(A, B, C, D);
impl_selection_output_tuple!(A, B, C, D, E);
impl_selection_output_tuple!(A, B, C, D, E, F);
impl_selection_output_tuple!(A, B, C, D, E, F, G);
impl_selection_output_tuple!(A, B, C, D, E, F, G, H);
impl_selection_output_tuple!(A, B, C, D, E, F, G, H, I);
impl_selection_output_tuple!(A, B, C, D, E, F, G, H, I, J);
impl_selection_output_tuple!(A, B, C, D, E, F, G, H, I, J, K);
impl_selection_output_tuple!(A, B, C, D, E, F, G, H, I, J, K, L);

#[macro_export]
macro_rules! take {
	($first:expr $(, $rest:expr)* $(,)?) => {{
		use $crate::select::{
			record_selection_expr as __sqlxo_record_selection_expr,
			SelectionEntry as __SqlxoSelectionEntry,
			SelectionList as __SqlxoSelectionList,
			SelectionOutputTuple as __SqlxoSelectionOutputTuple,
		};

		let mut __entries: $crate::smallvec::SmallVec<
			[__SqlxoSelectionEntry; 4]
		> = $crate::smallvec::SmallVec::new();

		let __outputs = (
			__sqlxo_record_selection_expr($first, &mut __entries),
			$(
				__sqlxo_record_selection_expr($rest, &mut __entries),
			)*
		);

		let __output_marker = __SqlxoSelectionOutputTuple::flatten(__outputs);
		__output_marker.into_selection_list(__entries)
	}};
}

#[macro_export]
macro_rules! group_by {
	($first:ty $(, $rest:ty)* $(,)?) => {{
		use $crate::select::{
			Column as __SqlxoColumn,
			GroupByList as __SqlxoGroupByList,
			SelectionColumn as __SqlxoSelectionColumn,
		};

		let mut __cols: $crate::smallvec::SmallVec<[__SqlxoSelectionColumn; 4]> =
			$crate::smallvec::SmallVec::new();
		__cols.push(__SqlxoSelectionColumn::new(
			<$first as __SqlxoColumn>::TABLE,
			<$first as __SqlxoColumn>::NAME,
		));
		$(
			__cols.push(__SqlxoSelectionColumn::new(
				<$rest as __SqlxoColumn>::TABLE,
				<$rest as __SqlxoColumn>::NAME,
			));
		)*

		__SqlxoGroupByList::new(__cols)
	}};
}

#[macro_export]
macro_rules! having {
	($first:expr $(, $rest:expr)* $(,)?) => {{
		let mut __preds = Vec::new();
		__preds.push($first);
		$(
			__preds.push($rest);
		)*
		$crate::select::HavingList::new(__preds)
	}};
}
