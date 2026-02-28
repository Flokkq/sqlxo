#![feature(inherent_associated_types)]
#![allow(incomplete_features)]

use criterion::{
	criterion_group,
	criterion_main,
	Criterion,
};
use sqlx::FromRow;
use sqlxo::blocks::{
	BuildableFilter,
	BuildableJoin,
};
use sqlxo::{
	Buildable,
	JoinKind,
	QueryBuilder,
};
use sqlxo::{
	JoinValue,
	Query,
};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow, Query)]
pub struct Material {
	#[primary_key]
	pub id: Uuid,
}

#[derive(Debug, Clone, FromRow, Query)]
pub struct Item {
	#[primary_key]
	pub id:          Uuid,
	#[foreign_key(to = "material.id")]
	pub material_id: Option<Uuid>,

	#[sqlxo(belongs_to)]
	#[sqlx(skip)]
	pub material: JoinValue<Material>,
}

fn bench_read_builder(c: &mut Criterion) {
	let mut group = c.benchmark_group("read_builder");

	group.bench_function("baseline", |b| {
		b.iter(|| {
			let plan = QueryBuilder::<Item>::read().build();
			criterion::black_box(plan.sql(sqlxo::blocks::SelectType::Star));
		});
	});

	group.bench_function("join_and_where", |b| {
		b.iter(|| {
			let plan = QueryBuilder::<Item>::read()
				.join(ItemJoin::ItemToMaterialByMaterialId, JoinKind::Left)
				.r#where(sqlxo::blocks::Expression::Leaf(
					ItemQuery::MaterialIdIsNotNull,
				))
				.build();
			criterion::black_box(plan.sql(sqlxo::blocks::SelectType::Star));
		});
	});

	group.finish();
}

criterion_group!(benches, bench_read_builder);
criterion_main!(benches);
