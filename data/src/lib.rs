pub use interface::*;
use parse::parse_config::{AliasConfig, CampaignConfig, Config, PlayerConfig};
use sqlx::{postgres::PgPoolOptions, query, Pool, Postgres, Transaction};
use std::env;

mod interface;

async fn get_postgres_url() -> String {
    env::var("DATABASE_URL").expect("failed to load DATABASE_URL environment variable")
}

pub async fn create_connection_pool(path_to_dotenv: &str) -> Pool<Postgres> {
    dotenv::from_path(path_to_dotenv)
        .expect("failed to find or load .env from provided path_to_dotenv");
    let connection_url = get_postgres_url().await;

    PgPoolOptions::new()
        .max_connections(5)
        .connect(&connection_url)
        .await
        .expect("failed to connect to database - check DATABASE_URL .env variable and ensure your database server is running")
}

pub async fn begin_transaction<'a>(pool: &'a Pool<Postgres>) -> Transaction<'a, Postgres> {
    pool.begin()
        .await
        .expect("failed to initiate database transaction")
}

async fn update_player_from_config<'a, 'tr>(
    transaction: &'a mut Transaction<'tr, Postgres>,
    player_name: &String,
    player_config: &PlayerConfig,
) -> i32 {
    let player = Player::from_values(player_name).await;
    let player_id = player.fetch_or_insert_id(&mut *transaction).await;

    let mut valid_pronouns_maps: Vec<i32> = vec![];
    for pronouns_string in &player_config.pronouns {
        let pronouns: Pronouns = pronouns_string[..].into();
        let pronouns_id = pronouns.fetch_or_insert_id(&mut *transaction).await;

        let pronouns_map_values = [pronouns_id, player_id];
        let pronouns_map = PronounsMap::from_values(&pronouns_map_values).await;
        let pronouns_map_id = pronouns_map.fetch_or_insert_id(&mut *transaction).await;
        valid_pronouns_maps.push(pronouns_map_id);
    }

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

    let player_pronouns_maps = query!(
        r#"SELECT pronouns_map.id
        FROM pronouns_map
        WHERE player_id = $1"#,
        player_id
    )
    .fetch_all(&mut **transaction)
    .await
    .expect("failed to fetch player pronouns from database");

    let player_censors = query!(
        r#"SELECT id
        FROM censor
        WHERE player_id = $1"#,
        player_id
    )
    .fetch_all(&mut **transaction)
    .await
    .expect("failed to fetch censors for player from database");

    for pronouns_map in player_pronouns_maps {
        let map_id = pronouns_map.id;
        if !valid_pronouns_maps.contains(&map_id) {
            query!(r#"DELETE FROM pronouns_map WHERE id = $1"#, map_id)
                .execute(&mut **transaction)
                .await
                .expect("failed to prune pronouns_map");
        }
    }

    for censor in player_censors {
        let censor_id = censor.id;
        if !valid_censors.contains(&censor_id) {
            query!(r#"DELETE FROM censor WHERE id = $1"#, censor_id)
                .execute(&mut **transaction)
                .await
                .expect("failed to prune censors");
        }
    }

    player_id
}

async fn sender_is_censored<'a, 'tr>(
    transaction: &'a mut Transaction<'tr, Postgres>,
    sender_name: &str,
    player_id: i32,
) -> bool {
    let censored_flags = query!(
        r#"SELECT avoid_text
        FROM censor
        WHERE
            player_id = $1 AND
            LOWER( $2 ) LIKE '%' || LOWER(censor.avoid_text) || '%'
        "#,
        player_id,
        sender_name,
    )
    .fetch_all(&mut **transaction)
    .await;

    match censored_flags {
        Ok(flags) if flags.len() > 0 => true,
        _ => false,
    }
}

async fn update_campaign_from_config<'a, 'tr>(
    transaction: &'a mut Transaction<'tr, Postgres>,
    campaign_name: &String,
    campaign_config: &CampaignConfig,
) -> i32 {
    let campaign_values = (
        campaign_name.clone(),
        campaign_config.dungeon_master.clone(),
        campaign_config.timezone_offset,
    );
    let campaign = Campaign::from_values(&campaign_values).await;
    let campaign_id = campaign.fetch_or_insert_id(&mut *transaction).await;

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
                sender_is_censored(&mut *transaction, sender, player_id).await,
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

    campaign_id
}

pub async fn update_players<'a, 'tr>(
    transaction: &'a mut Transaction<'tr, Postgres>,
    config: &Config,
) {
    let current_players = &config.players;

    let mut valid_players: Vec<i32> = vec![];
    for (player_name, player_config) in current_players {
        let player_id =
            update_player_from_config(&mut *transaction, player_name, player_config).await;
        valid_players.push(player_id);
    }

    let all_players = query!(
        r#"SELECT id
        FROM player"#
    )
    .fetch_all(&mut **transaction)
    .await
    .expect("failed to fetch player ids from database");

    for player in all_players {
        let player_id = player.id;
        if !valid_players.contains(&player_id) {
            query!(
                r#"DELETE FROM pronouns_map
                WHERE player_id = $1"#,
                player_id
            )
            .execute(&mut **transaction)
            .await
            .expect("failed to prune pronouns_map for stale player data");

            query!(
                r#"DELETE FROM player
                WHERE id = $1"#,
                player_id
            )
            .execute(&mut **transaction)
            .await
            .expect("failed to prune players");
        }
    }
}

pub async fn update_campaigns<'a, 'tr>(
    transaction: &'a mut Transaction<'tr, Postgres>,
    config: &Config,
) {
    let current_campaigns = &config.campaigns;

    let mut valid_campaigns: Vec<i32> = vec![];
    for (campaign_name, campaign_config) in current_campaigns {
        let campaign_id =
            update_campaign_from_config(&mut *transaction, campaign_name, campaign_config).await;
        valid_campaigns.push(campaign_id);
    }

    let all_campaigns: Vec<i32> = query!(
        r#"SELECT id
        FROM campaign"#
    )
    .fetch_all(&mut **transaction)
    .await
    .expect("failed to fetch campaign ids from database")
    .iter()
    .map(|rec| rec.id)
    .collect();

    for campaign_id in all_campaigns {
        if !valid_campaigns.contains(&campaign_id) {
            query!(
                r#"DELETE FROM alias
                    USING sender
                WHERE sender_id IN (
                    SELECT id
                    FROM sender
                    WHERE campaign_id = $1
                )"#,
                campaign_id
            )
            .execute(&mut **transaction)
            .await
            .expect("failed to prune alias table for stale campaign data");

            query!(
                r#"DELETE FROM sender
                WHERE campaign_id = $1"#,
                campaign_id
            )
            .execute(&mut **transaction)
            .await
            .expect("failed to prune sender table for stale campaign data");

            query!(
                r#"DELETE FROM campaign
                WHERE id = $1"#,
                campaign_id
            )
            .execute(&mut **transaction)
            .await
            .expect("failed to prune campaigns");
        }
    }
}
