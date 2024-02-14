use super::{IdInterface, ShapeInterface};
use sqlx::{query, Postgres, Transaction};

pub struct Campaign<'a> {
    pub campaign_name: &'a str,
    pub dm_name: &'a str,
    pub timezone_offset: i32,
}

impl<'a, 'tr> ShapeInterface<'a, 'tr> for Campaign<'a> {
    type Shape = (String, String, i32);

    async fn from_values(values_tuple: &'a Self::Shape) -> Self {
        Self {
            campaign_name: &values_tuple.0,
            dm_name: &values_tuple.1,
            timezone_offset: values_tuple.2,
        }
    }

    async fn try_fetch_values(
        transaction: &'a mut Transaction<'tr, Postgres>,
        id: i32,
    ) -> sqlx::Result<Self::Shape> {
        let joined = query!(
            r#"SELECT campaign_name, player_name, timezone_offset
            FROM campaign
                JOIN player
                ON dm_id = player.id
            WHERE campaign.id = $1"#,
            id
        )
        .fetch_one(&mut **transaction)
        .await?;

        Ok((
            joined.campaign_name,
            joined.player_name,
            joined.timezone_offset,
        ))
    }
}

impl<'a, 'tr> IdInterface<'a, 'tr> for Campaign<'a> {
    type IdType = i32;

    async fn try_fetch_id(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
    ) -> sqlx::Result<Self::IdType> {
        let id = query!(
            r#"SELECT campaign.id FROM campaign
                JOIN player ON dm_id = player.id
            WHERE player.player_name = $1
            AND campaign_name = $2"#,
            self.dm_name,
            self.campaign_name,
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
            r#"INSERT INTO campaign (campaign_name, dm_id, timezone_offset)
            SELECT $1 as campaign_name, (
                SELECT id
                FROM player
                WHERE player_name = $2
            ) as dm_id,
            $3 as timezone_offset
            ON CONFLICT (campaign_name) DO NOTHING
            RETURNING id"#,
            self.campaign_name,
            self.dm_name,
            self.timezone_offset,
        )
        .fetch_one(&mut **transaction)
        .await;

        match try_id {
            Ok(record) => Ok(record.id),
            Err(_) => {
                let id = query!(
                    r#"UPDATE campaign SET
                        dm_id = (
                            SELECT id
                            FROM player
                            WHERE player_name = $2
                        ),
                        timezone_offset = $3
                    WHERE campaign_name = $1
                    RETURNING id"#,
                    self.campaign_name,
                    self.dm_name,
                    self.timezone_offset,
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
            .expect("failed to insert new campaign record")
    }
}
