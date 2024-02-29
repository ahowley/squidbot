pub use fetch::*;
pub use interface::*;
use parse::{
    parse_config::{CampaignConfig, Config, PlayerConfig},
    ChatLog,
};
use sqlx::{postgres::PgPoolOptions, query, Pool, Postgres, Transaction};
use std::env;

mod fetch;
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

    player
        .update_and_prune_dependent_records(&mut *transaction, player_config, player_id)
        .await;

    player_id
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

    campaign
        .update_and_prune_dependent_records(&mut *transaction, campaign_config, campaign_id)
        .await;

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

// TODO: Ensure there's no way to improve this code with better polymorphism
pub async fn update_posts_from_log(
    pool: &Pool<Postgres>,
    campaign_name: &str,
    directory: &str,
    filename: &str,
    timezone_offset: Option<i32>,
) {
    let campaign_id = query!(
        r#"SELECT id FROM campaign WHERE campaign_name = $1"#,
        campaign_name
    )
    .fetch_one(pool)
    .await
    .expect("failed to fetch campaign id")
    .id;

    let mut fetch_ids_transaction = begin_transaction(pool).await;
    let already_parsed_hash = fetch_parsed_post_ids(&mut fetch_ids_transaction, campaign_id).await;
    fetch_ids_transaction
        .rollback()
        .await
        .expect("failed to rollback fetch transaction");

    let path_to_log = format!("{directory}/{filename}");

    if filename.starts_with("fnd_") {
        let mut log = parse::parse_foundry_log(&path_to_log, timezone_offset).await;

        while let Some(post) = log.next_post().await {
            if already_parsed_hash.contains(&post.id) {
                continue;
            }

            let transaction = begin_transaction(&pool).await;

            let mut interface = PostInterface {
                transaction,
                post,
                campaign_id,
            };

            interface.try_insert().await.unwrap_or(());
            interface
                .transaction
                .commit()
                .await
                .expect("failed to commit transaction");
        }
    } else if filename.starts_with("r20_") {
        let mut log = parse::parse_roll20_log(&path_to_log, timezone_offset).await;

        while let Some(post) = log.next_post().await {
            if already_parsed_hash.contains(&post.id) {
                continue;
            }

            let transaction = begin_transaction(&pool).await;

            let mut interface = PostInterface {
                transaction,
                post,
                campaign_id,
            };

            interface.try_insert().await.unwrap_or(());
            interface
                .transaction
                .commit()
                .await
                .expect("failed to commit transaction");
        }

        if cfg!(debug_assertions) {
            println!(
                "\
--Time spent parsing div depth: {:#?}
--Time spent parsing post fragments: {:#?}
--Time spent updating timestamps: {:#?}
--Time spent updating sender names: {:#?}",
                log.time_spent_parsing_div_depth,
                log.time_spent_parsing_fragments,
                log.time_spent_updating_datetime,
                log.time_spent_updating_sender
            );
        }
    } else if filename.starts_with("fg_") {
        let mut log = parse::parse_fantasy_grounds_log(&path_to_log, timezone_offset).await;

        while let Some(post) = log.next_post().await {
            if already_parsed_hash.contains(&post.id) {
                continue;
            }

            let transaction = begin_transaction(&pool).await;

            let mut interface = PostInterface {
                transaction,
                post,
                campaign_id,
            };

            interface.try_insert().await.unwrap_or(());
            interface
                .transaction
                .commit()
                .await
                .expect("failed to commit transaction");
        }
    }
}
