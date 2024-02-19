pub use interface::*;
use parse::{
    parse_config::{CampaignConfig, Config, PlayerConfig},
    ChatLog,
};
use rand::seq::SliceRandom;
use sqlx::{
    postgres::PgPoolOptions,
    query, query_as,
    types::chrono::{DateTime, FixedOffset, Utc},
    Pool, Postgres, Transaction,
};
use std::{collections::HashMap, env, fmt::Display, sync::Arc};

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

pub async fn update_posts_from_log(
    pool: Arc<Pool<Postgres>>,
    campaign_name: &str,
    path_to_log: String,
) {
    let mut log = parse::parse_log(path_to_log.to_string()).await;

    let campaign_id = query!(
        r#"SELECT id FROM campaign WHERE campaign_name = $1"#,
        campaign_name
    )
    .fetch_one(&*pool)
    .await
    .expect("failed to fetch campaign id")
    .id;

    while let Some(post) = log.next_post().await {
        let interface = PostInterface {
            pool: Arc::clone(&pool),
            post,
            campaign_id,
        };

        interface.try_insert().await.unwrap_or(());
    }
}

#[derive(PartialEq)]
struct UniqueSender {
    sender_name: String,
    campaign_name: String,
}

pub async fn dump_unmapped_senders(config: &Config) -> HashMap<String, Vec<String>> {
    let mapped_senders: Vec<UniqueSender> = config
        .campaigns
        .iter()
        .map(|(campaign_name, campaign_config)| {
            campaign_config
                .aliases
                .iter()
                .map(|alias| {
                    alias.senders.iter().map(|sender_name| UniqueSender {
                        sender_name: sender_name.clone(),
                        campaign_name: campaign_name.clone(),
                    })
                })
                .flatten()
        })
        .flatten()
        .collect();

    let mut unmapped_senders: Vec<UniqueSender> = vec![];
    for (campaign_name, campaign_config) in &config.campaigns {
        let path_to_log = format!("./chatlogs/{}", campaign_config.log);
        let mut log = parse::parse_log(path_to_log.to_string()).await;
        while let Some(post) = log.next_post().await {
            let sender = UniqueSender {
                sender_name: post.sender_name.clone(),
                campaign_name: campaign_name.clone(),
            };
            if !unmapped_senders.contains(&sender) && !mapped_senders.contains(&sender) {
                unmapped_senders.push(sender);
            }
        }
    }

    let mut sender_map = HashMap::<String, Vec<String>>::new();

    for UniqueSender {
        sender_name,
        campaign_name,
    } in unmapped_senders
    {
        if sender_map.get(&campaign_name).is_none() {
            let senders_vec: Vec<String> = vec![];
            sender_map.insert(campaign_name.clone(), senders_vec);
        }
        sender_map
            .get_mut(&campaign_name)
            .unwrap()
            .push(sender_name);
    }

    sender_map
}

pub async fn fetch_campaign_names(pool: &Pool<Postgres>) -> Vec<String> {
    query!(r#"SELECT campaign_name FROM campaign"#)
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|campaign| campaign.campaign_name)
        .collect()
}

pub async fn fetch_random_chat_message(pool: &Pool<Postgres>, campaign: Option<&str>) -> String {
    match campaign {
        Some(campaign_name) => {
            let messages: Vec<String> = query!(
                r#"SELECT content FROM chat_message
            JOIN post ON post_id = post.id
            JOIN campaign ON campaign_id = campaign.id
        WHERE
            campaign_name = $1"#,
                campaign_name
            )
            .fetch_all(pool)
            .await
            .unwrap()
            .into_iter()
            .map(|message| message.content)
            .collect();

            messages.choose(&mut rand::thread_rng()).unwrap().clone()
        }
        None => {
            let messages: Vec<String> = query!(r#"SELECT content FROM chat_message"#)
                .fetch_all(pool)
                .await
                .unwrap()
                .into_iter()
                .map(|message| message.content)
                .collect();

            messages.choose(&mut rand::thread_rng()).unwrap().clone()
        }
    }
}

pub struct MessageTrace {
    sender_name: String,
    player_name: String,
    campaign_name: String,
    timestamp_sent: DateTime<Utc>,
    timezone_offset: i32,
}

impl Display for MessageTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fixed_offset = FixedOffset::east_opt(self.timezone_offset * 3600).unwrap();
        let offset_timezone = self.timestamp_sent + fixed_offset;
        let date = offset_timezone.date_naive().format("%m/%d/%Y");
        let time = offset_timezone.time().format("%-I:%M %p");
        write!(
            f,
            "'{}' ({}) sent this on {} at {} in their \"{}\" campaign",
            self.sender_name, self.player_name, date, time, self.campaign_name
        )
    }
}

pub async fn trace_message(pool: &Pool<Postgres>, message: &str) -> Option<Vec<MessageTrace>> {
    let results: Vec<MessageTrace> = query_as!(
        MessageTrace,
        r#"SELECT sender_name, player_name, campaign_name, timestamp_sent, timezone_offset
        FROM alias
            JOIN sender ON alias.sender_id = sender.id
            JOIN player ON player_id = player.id
            JOIN post ON sender.id = post.sender_id
            JOIN chat_message ON post.id = post_id
            JOIN campaign ON post.campaign_id = campaign.id
        WHERE
            LOWER( $1 ) LIKE LOWER( content ) AND
            is_censored IS NOT TRUE"#,
        message
    )
    .fetch_all(pool)
    .await
    .unwrap_or(vec![]);

    if results.len() == 0 {
        return None;
    }

    Some(results)
}
