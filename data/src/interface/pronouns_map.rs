use super::{IdInterface, ShapeInterface};
use async_trait::async_trait;
use sqlx::{query, Postgres, Transaction};

pub struct PronounsMap {
    pub pronouns_id: i32,
    pub player_id: i32,
}

#[async_trait]
impl<'a, 'tr> ShapeInterface<'a, 'tr> for PronounsMap {
    type Shape = [i32; 2];

    async fn from_values(values: &'a Self::Shape) -> Self {
        Self {
            pronouns_id: values[0],
            player_id: values[1],
        }
    }

    async fn try_fetch_values(
        transaction: &'a mut Transaction<'tr, Postgres>,
        id: i32,
    ) -> sqlx::Result<Self::Shape> {
        let values = query!(
            r#"SELECT pronouns_id, player_id
            FROM pronouns_map
            WHERE id = $1"#,
            id
        )
        .fetch_one(&mut **transaction)
        .await?;

        Ok([values.pronouns_id, values.player_id])
    }
}

#[async_trait]
impl<'a, 'tr> IdInterface<'a, 'tr> for PronounsMap {
    type IdType = i32;

    async fn try_fetch_id(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"SELECT id FROM pronouns_map
            WHERE pronouns_id = $1 AND player_id = $2"#,
            self.pronouns_id,
            self.player_id
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
            r#"INSERT INTO pronouns_map (pronouns_id, player_id)
            VALUES ( $1, $2 )
            RETURNING id
            "#,
            self.pronouns_id,
            self.player_id,
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
            .expect("failed to insert new pronouns_map record")
    }
}
