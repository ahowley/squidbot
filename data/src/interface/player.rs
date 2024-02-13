use super::{IdInterface, ShapeInterface};
use sqlx::{query, Executor, Postgres, Transaction};

pub struct Player<'a> {
    pub player_name: &'a str,
}

impl<'a> ShapeInterface<'a> for Player<'a> {
    type Shape = String;

    async fn from_values(name: &'a String) -> Self {
        Player { player_name: &name }
    }

    async fn try_fetch_values<E: Executor<'a, Database = Postgres>>(
        pool: E,
        id: i32,
    ) -> sqlx::Result<Self::Shape> {
        let player_name = query!(r#"SELECT player_name FROM player WHERE id = $1"#, id)
            .fetch_one(pool)
            .await?
            .player_name;

        Ok(player_name)
    }
}

impl<'a> IdInterface<'a> for Player<'a> {
    type IdType = i32;

    async fn try_fetch_id<E: Executor<'a, Database = Postgres>>(
        &self,
        pool: E,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"SELECT id FROM player WHERE player_name = $1"#,
            self.player_name,
        )
        .fetch_one(pool)
        .await?
        .id;

        Ok(id)
    }

    async fn try_insert(
        &self,
        transaction: &mut Transaction<'a, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"INSERT INTO player (player_name)
            VALUES ( $1 )
            ON CONFLICT DO NOTHING
            RETURNING id
            "#,
            self.player_name
        )
        .fetch_one(&mut **transaction)
        .await?
        .id;

        Ok(id)
    }

    async fn fetch_or_insert_id(
        &self,
        transaction: &mut Transaction<'a, Postgres>,
    ) -> Self::IdType {
        if let Ok(id) = self.try_fetch_id(&mut **transaction).await {
            return id;
        }

        let id = self
            .try_insert(&mut *transaction)
            .await
            .expect("failed to insert new pronouns record");
        id
    }
}
