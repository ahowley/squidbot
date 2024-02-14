use super::{IdInterface, ShapeInterface};
use sqlx::{query, Postgres, Transaction};

pub struct Censor<'a> {
    pub avoid_text: &'a str,
    pub player_name: &'a str,
}

impl<'a, 'tr> ShapeInterface<'a, 'tr> for Censor<'a> {
    type Shape = [String; 2];

    async fn from_values(values_tuple: &'a Self::Shape) -> Self {
        Self {
            avoid_text: &values_tuple[0],
            player_name: &values_tuple[1],
        }
    }

    async fn try_fetch_values(
        transaction: &'a mut Transaction<'tr, Postgres>,
        id: i32,
    ) -> sqlx::Result<Self::Shape> {
        let joined = query!(
            r#"SELECT avoid_text, player_name
            FROM censor
                JOIN player
                ON player_id = player.id
            WHERE censor.id = $1"#,
            id
        )
        .fetch_one(&mut **transaction)
        .await?;

        Ok([joined.avoid_text, joined.player_name])
    }
}

impl<'a, 'tr> IdInterface<'a, 'tr> for Censor<'a> {
    type IdType = i32;

    async fn try_fetch_id(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"SELECT censor.id FROM censor
                JOIN player ON player_id = player.id
            WHERE player_name = $1
            AND avoid_text = $2"#,
            self.player_name,
            self.avoid_text,
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
            r#"INSERT INTO censor (avoid_text, player_id)
            SELECT $1 as avoid_text, (
                SELECT id
                FROM player
                WHERE player_name = $2
            ) as player_id
            ON CONFLICT DO NOTHING
            RETURNING id"#,
            self.avoid_text,
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
            .expect("failed to insert new censor record")
    }
}
