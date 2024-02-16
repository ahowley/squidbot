use super::{IdInterface, ShapeInterface};
use crate::{Censor, Pronouns, PronounsMap};
use parse::parse_config::PlayerConfig;
use sqlx::{query, Postgres, Transaction};

pub struct Player<'a> {
    pub player_name: &'a str,
}

impl<'a, 'tr> Player<'a> {
    async fn update_pronouns_maps(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
        player_config: &PlayerConfig,
        player_id: i32,
    ) -> Vec<i32> {
        let mut valid_pronouns_maps: Vec<i32> = vec![];
        for pronouns_string in &player_config.pronouns {
            let pronouns: Pronouns = pronouns_string[..].into();
            let pronouns_id = pronouns.fetch_or_insert_id(&mut *transaction).await;

            let pronouns_map_values = [pronouns_id, player_id];
            let pronouns_map = PronounsMap::from_values(&pronouns_map_values).await;
            let pronouns_map_id = pronouns_map.fetch_or_insert_id(&mut *transaction).await;
            valid_pronouns_maps.push(pronouns_map_id);
        }

        valid_pronouns_maps
    }

    async fn update_censors(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
        player_config: &PlayerConfig,
        player_id: i32,
    ) -> Vec<i32> {
        let mut valid_censors: Vec<i32> = vec![];
        for deadname in &player_config.deadnames {
            if deadname.len() == 0 {
                continue;
            }
            let censor_values = (deadname.clone(), player_id);
            let censor = Censor::from_values(&censor_values).await;
            let censor_id = censor.fetch_or_insert_id(&mut *transaction).await;
            valid_censors.push(censor_id);
        }

        valid_censors
    }

    async fn update_and_prune_pronouns_maps(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
        player_config: &PlayerConfig,
        player_id: i32,
    ) {
        let valid_pronouns_maps = self
            .update_pronouns_maps(&mut *transaction, player_config, player_id)
            .await;
        let player_pronouns_maps = query!(
            r#"SELECT pronouns_map.id
        FROM pronouns_map
        WHERE player_id = $1"#,
            player_id
        )
        .fetch_all(&mut **transaction)
        .await
        .expect("failed to fetch player pronouns from database");

        for pronouns_map in player_pronouns_maps {
            let map_id = pronouns_map.id;
            if !valid_pronouns_maps.contains(&map_id) {
                query!(r#"DELETE FROM pronouns_map WHERE id = $1"#, map_id)
                    .execute(&mut **transaction)
                    .await
                    .expect("failed to prune pronouns_map");
            }
        }
    }

    async fn update_and_prune_censors(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
        player_config: &PlayerConfig,
        player_id: i32,
    ) {
        let valid_censors = self
            .update_censors(&mut *transaction, player_config, player_id)
            .await;
        let player_censors = query!(
            r#"SELECT id
        FROM censor
        WHERE player_id = $1"#,
            player_id
        )
        .fetch_all(&mut **transaction)
        .await
        .expect("failed to fetch censors for player from database");

        for censor in player_censors {
            let censor_id = censor.id;
            if !valid_censors.contains(&censor_id) {
                query!(r#"DELETE FROM censor WHERE id = $1"#, censor_id)
                    .execute(&mut **transaction)
                    .await
                    .expect("failed to prune censors");
            }
        }
    }

    pub async fn update_and_prune_dependent_records(
        &self,
        transaction: &'a mut Transaction<'tr, Postgres>,
        player_config: &PlayerConfig,
        player_id: i32,
    ) {
        self.update_and_prune_pronouns_maps(&mut *transaction, player_config, player_id)
            .await;
        self.update_and_prune_censors(&mut *transaction, player_config, player_id)
            .await;
    }
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
