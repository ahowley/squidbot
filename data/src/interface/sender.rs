use super::{IdInterface, ShapeInterface};
use sqlx::{query, Postgres, Transaction};

pub struct Sender<'a> {
    pub sender_name: &'a str,
    pub campaign_id: i32,
    pub is_censored: bool,
}

impl<'a, 'tr> ShapeInterface<'a, 'tr> for Sender<'a> {
    type Shape = (String, i32, bool);

    async fn from_values(values_tuple: &'a Self::Shape) -> Self {
        Self {
            sender_name: &values_tuple.0,
            campaign_id: values_tuple.1,
            is_censored: values_tuple.2,
        }
    }

    async fn try_fetch_values(
        transaction: &'a mut Transaction<'tr, Postgres>,
        id: i32,
    ) -> sqlx::Result<Self::Shape> {
        let values = query!(
            r#"SELECT sender_name, campaign_id, is_censored
            FROM sender
            WHERE id = $1"#,
            id
        )
        .fetch_one(&mut **transaction)
        .await?;

        Ok((values.sender_name, values.campaign_id, values.is_censored))
    }
}

impl<'a, 'tr> IdInterface<'a, 'tr> for Sender<'a> {
    type IdType = i32;

    async fn try_fetch_id(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"SELECT id FROM sender
            WHERE sender_name = $1 AND campaign_id = $2 AND is_censored = $3"#,
            self.sender_name,
            self.campaign_id,
            self.is_censored,
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
        let try_id = query!(
            r#"INSERT INTO sender (sender_name, campaign_id, is_censored)
            VALUES ( $1, $2, $3 )
            ON CONFLICT DO NOTHING
            RETURNING id
            "#,
            self.sender_name,
            self.campaign_id,
            self.is_censored,
        )
        .fetch_one(&mut **transaction)
        .await;

        match try_id {
            Ok(record) => Ok(record.id),
            Err(_) => {
                let id = query!(
                    r#"UPDATE sender SET
                        is_censored = $3
                    WHERE sender_name = $1 AND
                    campaign_id = $2
                    RETURNING id"#,
                    self.sender_name,
                    self.campaign_id,
                    self.is_censored,
                )
                .fetch_one(&mut **transaction)
                .await?
                .id;

                Ok(id)
            }
        }
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
            .expect("failed to insert new sender record")
    }
}
