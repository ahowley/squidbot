use parse::{parse_config::Config, ChatLog};
use rand::seq::SliceRandom;
use sqlx::{
    query, query_as,
    types::chrono::{DateTime, FixedOffset, Utc},
    Pool, Postgres,
};
use std::{collections::HashMap, fmt::Display};

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

        if campaign_config.log.starts_with("fnd_") {
            let mut log = parse::parse_foundry_log(path_to_log.to_string(), None).await;
            while let Some(post) = log.next_post().await {
                let sender = UniqueSender {
                    sender_name: post.sender_name.clone(),
                    campaign_name: campaign_name.clone(),
                };
                if !unmapped_senders.contains(&sender)
                    && !mapped_senders.contains(&sender)
                    && sender.sender_name.len() > 0
                {
                    unmapped_senders.push(sender);
                }
            }
        }

        if campaign_config.log.starts_with("r20_") {
            let mut log = parse::parse_roll20_log(path_to_log.to_string(), None).await;
            while let Some(post) = log.next_post().await {
                let sender = UniqueSender {
                    sender_name: post.sender_name.clone(),
                    campaign_name: campaign_name.clone(),
                };
                if !unmapped_senders.contains(&sender)
                    && !mapped_senders.contains(&sender)
                    && sender.sender_name.len() > 0
                {
                    unmapped_senders.push(sender);
                }
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

pub async fn fetch_sender_names(pool: &Pool<Postgres>) -> Vec<String> {
    query!(
        r#"SELECT DISTINCT sender_name FROM sender
           WHERE is_censored IS NOT true"#
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|sender| sender.sender_name)
    .collect()
}

pub async fn fetch_player_names(pool: &Pool<Postgres>) -> Vec<String> {
    query!(r#"SELECT player_name FROM player"#)
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|player| player.player_name)
        .collect()
}

pub async fn fetch_random_chat_message(
    pool: &Pool<Postgres>,
    config: &Config,
    campaign: &str,
    sender: &str,
    player: &str,
) -> String {
    let messages: Vec<String> = query!(
        r#"SELECT content FROM chat_message
            JOIN post ON chat_message.post_id = post.id
            JOIN campaign ON post.campaign_id = campaign.id
            JOIN sender ON post.sender_id = sender.id
            JOIN alias ON sender.id = alias.sender_id
            JOIN player ON alias.player_id = player.id
        WHERE
            LOWER(campaign_name) LIKE LOWER('%' || $1 || '%') AND
            LOWER(sender_name) LIKE LOWER('%' || $2 || '%') AND
            LOWER(player_name) LIKE LOWER('%' || $3 || '%')"#,
        campaign,
        sender,
        player
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|message| message.content)
    .collect();

    let censored_text: Vec<String> = query!(r#"SELECT avoid_text FROM censor"#)
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|message| message.avoid_text)
        .collect();

    let mut random_message = messages
        .choose(&mut rand::thread_rng())
        .unwrap_or(&"".to_string())
        .clone();

    for censored in censored_text {
        random_message = random_message.replace(
            censored.as_str(),
            config.replace_all_deadnames_with.as_str(),
        );
    }

    random_message
}

pub struct MessageTrace {
    id: String,
    sender_name: String,
    is_censored: bool,
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

        if self.is_censored {
            write!(
                f,
                "Message ID: {}\n{} sent this on {} at {} in \"{}\"",
                self.id, self.player_name, date, time, self.campaign_name
            )
        } else {
            write!(
                f,
                "Message ID: {}\n{} ('{}') sent this on {} at {} in \"{}\"",
                self.id, self.player_name, self.sender_name, date, time, self.campaign_name
            )
        }
    }
}

pub async fn trace_message(pool: &Pool<Postgres>, message: &str) -> Option<Vec<MessageTrace>> {
    let results: Vec<MessageTrace> = query_as!(
        MessageTrace,
        r#"SELECT post.id, sender_name, is_censored, player_name, campaign_name, timestamp_sent, timezone_offset
        FROM alias
            JOIN sender ON alias.sender_id = sender.id
            JOIN player ON player_id = player.id
            JOIN post ON sender.id = post.sender_id
            JOIN chat_message ON post.id = post_id
            JOIN campaign ON post.campaign_id = campaign.id
        WHERE
            LOWER(content) LIKE LOWER( $1 )"#,
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

pub struct FullMessageTrace {
    sender_name: String,
    is_censored: bool,
    player_name: String,
    campaign_name: String,
    timestamp_sent: DateTime<Utc>,
    timezone_offset: i32,
    content: String,
}

impl Display for FullMessageTrace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let fixed_offset = FixedOffset::east_opt(self.timezone_offset * 3600).unwrap();
        let offset_timezone = self.timestamp_sent + fixed_offset;
        let date = offset_timezone.date_naive().format("%m/%d/%Y");
        let time = offset_timezone.time().format("%-I:%M %p");

        if self.is_censored {
            write!(
                f,
                "{} sent this on {} at {} in \"{}\":\n{}",
                self.player_name, date, time, self.campaign_name, self.content
            )
        } else {
            write!(
                f,
                "{} ('{}') sent this on {} at {} in \"{}\":\n{}",
                self.player_name, self.sender_name, date, time, self.campaign_name, self.content
            )
        }
    }
}

pub async fn trace_around_message(
    pool: &Pool<Postgres>,
    config: &Config,
    message_id: &str,
    num_around: i32,
) -> Option<Vec<FullMessageTrace>> {
    let campaign_id = query!(
        r#"SELECT campaign.id FROM post
            JOIN campaign ON campaign_id = campaign.id
        WHERE
            post.id = $1"#,
        message_id
    )
    .fetch_one(pool)
    .await
    .ok()?
    .id;

    let results: Vec<FullMessageTrace> = query!(
        r#"SELECT
            sender_name,
            is_censored,
            player_name,
            campaign_name,
            timestamp_sent,
            timezone_offset,
            content
        FROM (
            SELECT
                sender_name,
                is_censored,
                player_name,
                campaign_name,
                timestamp_sent,
                timezone_offset,
                content
            FROM alias
                JOIN sender ON alias.sender_id = sender.id
                JOIN player ON player_id = player.id
                JOIN post ON sender.id = post.sender_id
                JOIN chat_message ON post.id = post_id
                JOIN campaign ON post.campaign_id = campaign.id
            WHERE timestamp_sent <= (
                SELECT timestamp_sent
                FROM post
                WHERE id = $1
                ORDER BY timestamp_sent ASC
                LIMIT 1
            )
            AND campaign.id = $2
            ORDER BY timestamp_sent DESC
            LIMIT 1 + $3
        )
        UNION
        (
            SELECT
                sender_name,
                is_censored,
                player_name,
                campaign_name,
                timestamp_sent,
                timezone_offset,
                content
            FROM alias
                JOIN sender ON alias.sender_id = sender.id
                JOIN player ON player_id = player.id
                JOIN post ON sender.id = post.sender_id
                JOIN chat_message ON post.id = post_id
                JOIN campaign ON post.campaign_id = campaign.id
            WHERE timestamp_sent > (
                SELECT timestamp_sent
                FROM post
                WHERE id = $1
                ORDER BY timestamp_sent ASC
                LIMIT 1
            )
            AND campaign.id = $2
            ORDER BY timestamp_sent ASC
            LIMIT $3
        )
        ORDER BY timestamp_sent ASC"#,
        message_id,
        campaign_id,
        num_around as i64
    )
    .fetch_all(pool)
    .await
    .unwrap_or(vec![])
    .into_iter()
    .map(|rec| FullMessageTrace {
        sender_name: rec.sender_name.unwrap(),
        is_censored: rec.is_censored.unwrap(),
        player_name: rec.player_name.unwrap(),
        campaign_name: rec.campaign_name.unwrap(),
        timestamp_sent: rec.timestamp_sent.unwrap(),
        timezone_offset: rec.timezone_offset.unwrap(),
        content: rec.content.unwrap(),
    })
    .collect();

    let censored_text: Vec<String> = query!(r#"SELECT avoid_text FROM censor"#)
        .fetch_all(pool)
        .await
        .unwrap()
        .into_iter()
        .map(|message| message.avoid_text)
        .collect();

    let censored_results: Vec<FullMessageTrace> = results
        .into_iter()
        .map(|trace| {
            let censored_in_content: Vec<&String> = censored_text
                .iter()
                .filter(|text| {
                    trace
                        .content
                        .to_lowercase()
                        .contains(text.to_lowercase().as_str())
                })
                .collect();

            let mut censored_result = trace.content;
            for censored in censored_in_content {
                censored_result =
                    censored_result.replace(censored, config.replace_all_deadnames_with.as_str());
            }

            FullMessageTrace {
                content: censored_result,
                ..trace
            }
        })
        .collect();

    if censored_results.len() == 0 {
        return None;
    }

    Some(censored_results)
}
