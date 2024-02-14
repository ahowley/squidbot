use super::{IdInterface, ShapeInterface};
use sqlx::{query, Postgres, Transaction};

pub struct Censor<'a> {
    pub avoid_text: &'a str,
    pub player_id: i32,
}

impl<'a, 'tr> ShapeInterface<'a, 'tr> for Censor<'a> {
    type Shape = (String, i32);

    async fn from_values(values_tuple: &'a Self::Shape) -> Self {
        Self {
            avoid_text: &values_tuple.0,
            player_id: values_tuple.1,
        }
    }

    async fn try_fetch_values(
        transaction: &'a mut Transaction<'tr, Postgres>,
        id: i32,
    ) -> sqlx::Result<Self::Shape> {
        let values = query!(
            r#"SELECT avoid_text, player_id
            FROM censor
            WHERE id = $1"#,
            id
        )
        .fetch_one(&mut **transaction)
        .await?;

        Ok((values.avoid_text, values.player_id))
    }
}

impl<'a, 'tr> IdInterface<'a, 'tr> for Censor<'a> {
    type IdType = i32;

    async fn try_fetch_id(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"SELECT id FROM censor
            WHERE avoid_text = $1 AND player_id = $2"#,
            self.avoid_text,
            self.player_id,
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
            VALUES ( $1, $2 )
            RETURNING id"#,
            self.avoid_text,
            self.player_id
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
