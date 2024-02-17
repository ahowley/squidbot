use super::{IdInterface, ShapeInterface};
use crate::{Alias, Player, Sender};
use async_trait::async_trait;
use parse::parse_config::{AliasConfig, CampaignConfig};
use sqlx::{query, Postgres, Transaction};

pub struct Campaign<'a> {
    pub campaign_name: &'a str,
    pub dm_name: &'a str,
    pub timezone_offset: i32,
}

impl<'a, 'tr> Campaign<'a> {
    async fn update_senders_and_aliases(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
        campaign_config: &CampaignConfig,
        campaign_id: i32,
    ) -> (Vec<i32>, Vec<i32>) {
        let mut valid_senders: Vec<i32> = vec![];
        let mut valid_aliases: Vec<i32> = vec![];
        for AliasConfig { player, senders } in &campaign_config.aliases {
            let player = Player::from_values(player).await;
            let player_id = player.try_fetch_id(&mut *transaction).await.expect(
            "failed to fetch player id while parsing campaign - ensure players are updated first",
        );

            for sender in senders {
                let sender_values = (
                    sender.clone(),
                    campaign_id,
                    Sender::is_censored(&mut *transaction, sender, player_id).await,
                );
                let sender = Sender::from_values(&sender_values).await;
                let sender_id = sender.fetch_or_insert_id(&mut *transaction).await;
                valid_senders.push(sender_id);

                let alias_values = [sender_id, player_id];
                let alias = Alias::from_values(&alias_values).await;
                let alias_id = alias.fetch_or_insert_id(&mut *transaction).await;
                valid_aliases.push(alias_id);
            }
        }

        (valid_senders, valid_aliases)
    }

    async fn update_and_prune_senders_and_aliases(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
        campaign_config: &CampaignConfig,
        campaign_id: i32,
    ) {
        let (valid_senders, valid_aliases) = self
            .update_senders_and_aliases(&mut *transaction, campaign_config, campaign_id)
            .await;
        let campaign_senders: Vec<i32> = query!(
            r#"SELECT id
        FROM sender
        WHERE campaign_id = $1"#,
            campaign_id
        )
        .fetch_all(&mut **transaction)
        .await
        .expect("failed to fetch player pronouns from database")
        .iter()
        .map(|rec| rec.id)
        .collect();

        let mut campaign_aliases: Vec<i32> = vec![];
        for sender_id in &campaign_senders {
            query!(
                r#"SELECT alias.id
            FROM alias
                JOIN sender ON sender_id = sender.id
            WHERE
                sender.id = $1"#,
                sender_id,
            )
            .fetch_all(&mut **transaction)
            .await
            .unwrap()
            .iter()
            .for_each(|rec| campaign_aliases.push(rec.id));
        }

        for alias_id in campaign_aliases {
            if !valid_aliases.contains(&alias_id) {
                query!(r#"DELETE FROM alias WHERE id = $1"#, alias_id)
                    .execute(&mut **transaction)
                    .await
                    .expect("failed to prune alias table");
            }
        }

        for sender_id in campaign_senders {
            if !valid_senders.contains(&sender_id) {
                query!(r#"DELETE FROM sender WHERE id = $1"#, sender_id)
                    .execute(&mut **transaction)
                    .await
                    .expect("failed to prune sender table");
            }
        }
    }

    pub async fn update_and_prune_dependent_records(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
        campaign_config: &CampaignConfig,
        campaign_id: i32,
    ) {
        self.update_and_prune_senders_and_aliases(&mut *transaction, campaign_config, campaign_id)
            .await;
    }
}

#[async_trait]
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

#[async_trait]
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
