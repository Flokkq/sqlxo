pub trait BuildableInsertQuery<C>: Buildable<C, Plan: Planable<C>>
where
	C: QueryContext,
{
}
