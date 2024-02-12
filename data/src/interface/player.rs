use crate::{GeneratedIdTransaction, ShapeInterface};
use sqlx::{query, Executor, Pool, Postgres};

pub struct Player<'a> {
    pub player_name: &'a str,
}

impl<'a> ShapeInterface<'a> for Player<'a> {
    type Shape = String;

    async fn from_values(name: &'a Self::Shape) -> Self {
        Player { player_name: name }
    }

    async fn fetch_values<E: Executor<'a, Database = Postgres>>(
        pool: E,
        id: i32,
    ) -> sqlx::Result<Self::Shape> {
        let player_name = query!(r#"SELECT player_name FROM player WHERE id = $1"#, id)
            .fetch_one(pool)
            .await?
            .player_name;

        Ok(player_name)
    }

    async fn fetch_id_by_values<E: Executor<'a, Database = Postgres>>(
        pool: E,
        values: &Self,
    ) -> sqlx::Result<i32> {
        let id = query!(
            r#"SELECT id FROM player WHERE player_name = $1"#,
            values.player_name
        )
        .fetch_one(pool)
        .await?
        .id;

        Ok(id)
    }

    async fn try_insert(
        &self,
        pool: &'a Pool<Postgres>,
    ) -> sqlx::Result<GeneratedIdTransaction<'a>> {
        let mut transaction = pool.begin().await?;

        let id = query!(
            r#"INSERT INTO player (player_name) VALUES ( $1 ) RETURNING id"#,
            self.player_name
        )
        .fetch_one(&mut *transaction)
        .await?
        .id;

        Ok(GeneratedIdTransaction(transaction, id))
    }
}
