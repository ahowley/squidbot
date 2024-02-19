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
