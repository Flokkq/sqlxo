pub trait BuildableUpdateQuery<C>:
	Buildable<C, Plan: Planable<C>> + BuildableFilter<C>
where
	C: QueryContext,
{
}
