pub use player::Player;
pub use pronouns::Pronouns;
pub use pronouns_map::PronounsMap;
use sqlx::{Pool, Postgres, Transaction};

mod player;
mod pronouns;
mod pronouns_map;

#[allow(async_fn_in_trait)]
pub trait ShapeInterface<'a, 'tr>
where
    Self: Sized,
{
    type Shape;

    async fn from_values(value: &'a Self::Shape) -> Self;

    async fn try_fetch_values(
        transaction: &'a mut Transaction<'tr, Postgres>,
        id: i32,
    ) -> sqlx::Result<Self::Shape>;
}

#[allow(async_fn_in_trait)]
pub trait IdInterface<'a, 'tr> {
    type IdType;

    async fn try_fetch_id(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType>;

    async fn try_insert(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType>;

    async fn fetch_or_insert_id(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> Self::IdType;
}

#[allow(async_fn_in_trait)]
pub trait UpdatableInterface<'a, 'tr>: IdInterface<'a, 'tr> {
    async fn insert_or_update(&self, pool: &'a Pool<Postgres>) -> Self::IdType;
}
