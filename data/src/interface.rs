pub use player::Player;
pub use pronouns::Pronouns;
use sqlx::{Executor, Pool, Postgres, Transaction};

mod player;
mod pronouns;

#[allow(async_fn_in_trait)]
pub trait ShapeInterface<'a>
where
    Self: Sized,
{
    type Shape;

    async fn from_values(value: &'a Self::Shape) -> Self;

    async fn try_fetch_values<E: Executor<'a, Database = Postgres>>(
        pool: E,
        id: i32,
    ) -> sqlx::Result<Self::Shape>;
}

#[allow(async_fn_in_trait)]
pub trait IdInterface<'a> {
    type IdType;

    async fn try_fetch_id<E: Executor<'a, Database = Postgres>>(
        &self,
        pool: E,
    ) -> sqlx::Result<Self::IdType>;

    async fn try_insert(
        &self,
        transaction: &mut Transaction<'a, Postgres>,
    ) -> sqlx::Result<Self::IdType>;

    async fn fetch_or_insert_id(&self, transaction: &mut Transaction<'a, Postgres>)
        -> Self::IdType;
}

#[allow(async_fn_in_trait)]
pub trait UpdatableInterface<'a>: IdInterface<'a> {
    async fn insert_or_update(&self, pool: &'a Pool<Postgres>) -> Self::IdType;
}
