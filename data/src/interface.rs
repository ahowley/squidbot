use sqlx::{Executor, Pool, Postgres, Transaction};

#[allow(async_fn_in_trait)]
pub trait ShapeInterface<'a>
where
    Self: Sized,
{
    type Shape;

    async fn from_values(values: &'a Self::Shape) -> Self;

    async fn fetch_values<E: Executor<'a, Database = Postgres>>(
        pool: E,
        id: i32,
    ) -> sqlx::Result<Self::Shape>;

    async fn fetch_id_by_values<E: Executor<'a, Database = Postgres>>(
        pool: E,
        values: &Self,
    ) -> sqlx::Result<i32>;

    async fn try_insert(
        &self,
        pool: &'a Pool<Postgres>,
    ) -> sqlx::Result<GeneratedIdTransaction<'a>>;
}

pub struct GeneratedIdTransaction<'a>(pub Transaction<'a, Postgres>, pub i32);
