use super::{IdInterface, ShapeInterface};
use sqlx::{query, Postgres, Transaction};

pub struct Player<'a> {
    pub player_name: &'a str,
}

impl<'a, 'tr> ShapeInterface<'a, 'tr> for Player<'a> {
    type Shape = String;

    async fn from_values(name: &'a String) -> Self {
        Self { player_name: name }
    }

    async fn try_fetch_values(
        transaction: &'a mut Transaction<'tr, Postgres>,
        id: i32,
    ) -> sqlx::Result<Self::Shape> {
        let player_name = query!(r#"SELECT player_name FROM player WHERE id = $1"#, id)
            .fetch_one(&mut **transaction)
            .await?
            .player_name;

        Ok(player_name)
    }
}

impl<'a, 'tr> IdInterface<'a, 'tr> for Player<'a> {
    type IdType = i32;

    async fn try_fetch_id(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"SELECT id FROM player WHERE player_name = $1"#,
            self.player_name,
        )
        .fetch_one(&mut **transaction)
        .await?
        .id;

        Ok(id)
    }

    async fn try_insert(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"INSERT INTO player (player_name)
            VALUES ( $1 )
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
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> Self::IdType {
        if let Ok(id) = self.try_fetch_id(&mut *transaction).await {
            return id;
        }

        self.try_insert(&mut *transaction)
            .await
            .expect("failed to insert new pronouns record")
    }
}
