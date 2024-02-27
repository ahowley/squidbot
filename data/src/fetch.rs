use parse::{parse_config::Config, ChatLog, RollSingle};
use rand::seq::SliceRandom;
use sqlx::{
    query, query_as,
    types::chrono::{DateTime, FixedOffset, Utc},
    Pool, Postgres,
};
use std::collections::{HashMap, HashSet};

#[derive(PartialEq, Eq, Hash)]
struct UniqueSender {
    sender_name: String,
    campaign_name: String,
}

pub async fn dump_unmapped_senders(config: &Config) -> HashMap<String, Vec<String>> {
    let senders = config
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
        .flatten();
    let mut senders_hash = HashSet::new();
    senders.for_each(|sender| {
        senders_hash.insert(sender);
    });

    let mut unmapped_senders: Vec<UniqueSender> = vec![];
    for (campaign_name, campaign_config) in &config.campaigns {
        let path_to_log = format!("./chatlogs/{}", campaign_config.log);

        if campaign_config.log.starts_with("fnd_") {
            let mut log = parse::parse_foundry_log(&path_to_log, None).await;
            while let Some(post) = log.next_post().await {
                let sender = UniqueSender {
                    sender_name: post.sender_name.clone(),
                    campaign_name: campaign_name.clone(),
                };
                if !unmapped_senders.contains(&sender)
                    && !senders_hash.contains(&sender)
                    && sender.sender_name.len() > 0
                {
                    unmapped_senders.push(sender);
                }
            }
        }

        if campaign_config.log.starts_with("r20_") {
            let mut log = parse::parse_roll20_log(&path_to_log, None).await;
            while let Some(post) = log.next_post().await {
                let sender = UniqueSender {
                    sender_name: post.sender_name.clone(),
                    campaign_name: campaign_name.clone(),
                };
                if !unmapped_senders.contains(&sender)
                    && !senders_hash.contains(&sender)
                    && sender.sender_name.len() > 0
                {
                    unmapped_senders.push(sender);
                }
            }
        }

        if campaign_config.log.starts_with("fg_") {
            let mut log = parse::parse_fantasy_grounds_log(&path_to_log, None).await;
            while let Some(post) = log.next_post().await {
                let sender = UniqueSender {
                    sender_name: post.sender_name.clone(),
                    campaign_name: campaign_name.clone(),
                };
                if !unmapped_senders.contains(&sender)
                    && !senders_hash.contains(&sender)
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

pub async fn fetch_player_pronouns(pool: &Pool<Postgres>, player_name: &str) -> Vec<[String; 4]> {
    query!(
        r#"SELECT subj, obj, poss_pres, poss_past FROM pronouns
            JOIN pronouns_map ON pronouns.id = pronouns_id
            JOIN player ON player_id = player.id
        WHERE
            player_name = $1"#,
        player_name
    )
    .fetch_all(pool)
    .await
    .unwrap()
    .into_iter()
    .map(|pronouns| {
        [
            pronouns.subj,
            pronouns.obj,
            pronouns.poss_pres,
            pronouns.poss_past,
        ]
    })
    .collect()
}

pub async fn fetch_all_single_rolls(pool: &Pool<Postgres>, player_name: &str) -> Vec<RollSingle> {
    query_as!(
        RollSingle,
        r#"SELECT faces, roll_single.outcome FROM roll_single
            JOIN roll ON roll_single.roll_id = roll.id
            JOIN post ON roll.post_id = post.id
            JOIN sender ON post.sender_id = sender.id
            JOIN alias ON sender.id = alias.sender_id
            JOIN player ON alias.player_id = player.id
        WHERE
            player_name = $1"#,
        player_name
    )
    .fetch_all(pool)
    .await
    .unwrap_or(vec![])
}

pub async fn fetch_all_parseable_rolls(
    pool: &Pool<Postgres>,
) -> Vec<(String, String, String, f64, DateTime<Utc>, i32)> {
    query!(
        r#"SELECT player_name, campaign_name, formula, outcome, timestamp_sent, timezone_offset FROM roll
            JOIN post ON roll.post_id = post.id
            JOIN sender ON post.sender_id = sender.id
            JOIN campaign ON sender.campaign_id = campaign.id
            JOIN alias ON sender.id = alias.sender_id
            JOIN player ON alias.player_id = player.id
        WHERE
            formula NOT LIKE '%k%' AND
            formula NOT LIKE '%ro%' AND
            formula NOT LIKE '%dF%' AND
            formula LIKE '%d%'"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or(vec![])
    .into_iter()
    .map(|rec| {
        (
            rec.player_name,
            rec.campaign_name,
            rec.formula,
            rec.outcome,
            rec.timestamp_sent,
            rec.timezone_offset
        )
    })
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

    let mut random_message = messages
        .choose(&mut rand::thread_rng())
        .unwrap_or(&"".to_string())
        .clone();
    random_message = censor_text(
        &random_message,
        &fetch_censored_phrases(pool).await,
        &config.replace_all_deadnames_with,
    );

    random_message
}

async fn fetch_censored_phrases(pool: &Pool<Postgres>) -> Vec<String> {
    query!(r#"SELECT avoid_text FROM censor"#)
        .fetch_all(pool)
        .await
        .unwrap_or(vec![])
        .into_iter()
        .map(|message| message.avoid_text)
        .collect()
}

fn censor_text(text: &str, censored_phrases: &[String], replace_with: &str) -> String {
    let mut censored_text = text.to_string();
    for phrase in censored_phrases {
        let phrase_lower = &phrase.to_lowercase();
        if let Some(censored_start) = censored_text.to_lowercase().find(phrase_lower) {
            let censored_end = censored_start + phrase.len() - 1;
            let char_before_censored = censored_text.chars().nth(censored_start - 1).unwrap_or(' ');
            let char_after_censored = censored_text.chars().nth(censored_end + 1).unwrap_or(' ');
            if char_before_censored == ' ' && char_after_censored == ' ' {
                let censored_phrase_as_appears = &censored_text[censored_start..censored_end + 1];
                censored_text = censored_text.replace(censored_phrase_as_appears, replace_with);
            }
        }
    }

    censored_text
}

pub struct MessageTrace {
    id: String,
    sender_name: String,
    is_censored: bool,
    player_name: String,
    campaign_name: String,
    timestamp_sent: DateTime<Utc>,
    timezone_offset: i32,
    content: String,
}

impl MessageTrace {
    pub fn as_message(&self, with_id: bool, with_content: bool) -> String {
        let fixed_offset = FixedOffset::east_opt(self.timezone_offset * 3600).unwrap();
        let offset_timezone = self.timestamp_sent + fixed_offset;
        let date = offset_timezone.date_naive().format("%m/%d/%Y");
        let time = offset_timezone.time().format("%-I:%M %p");

        let mut message = if with_id {
            format!("Message ID: {}\n", self.id)
        } else {
            String::new()
        };

        let safe_name = if self.is_censored {
            &self.player_name
        } else {
            &self.sender_name
        };

        if with_content {
            message.push_str(&format!(
                "{} in \"{}\" [{} {}]: {}",
                safe_name, self.campaign_name, date, time, self.content
            ));
        } else {
            message.push_str(&format!(
                "{} sent this on {} at {} in \"{}\"",
                safe_name, date, time, self.campaign_name
            ));
        }

        message
    }
}

pub async fn trace_message(pool: &Pool<Postgres>, message: &str) -> Option<Vec<MessageTrace>> {
    let results: Vec<MessageTrace> = query_as!(
        MessageTrace,
        r#"SELECT
            post.id,
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
        WHERE
            LOWER(content) = LOWER( $1 )"#,
        message.trim()
    )
    .fetch_all(pool)
    .await
    .ok()?;

    if results.len() == 0 {
        return None;
    }

    Some(results)
}

pub async fn search_for_message(
    pool: &Pool<Postgres>,
    config: &Config,
    message: &str,
    limit: i32,
) -> Option<Vec<MessageTrace>> {
    let censored_phrases = fetch_censored_phrases(pool).await;
    let results: Vec<MessageTrace> = query_as!(
        MessageTrace,
        r#"SELECT
            post.id,
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
        WHERE
            LOWER(content) LIKE '%' || LOWER( $1 ) || '%'
        ORDER BY
            timestamp_sent DESC
        LIMIT $2"#,
        message.trim(),
        limit as i64
    )
    .fetch_all(pool)
    .await
    .ok()?
    .into_iter()
    .map(|trace| MessageTrace {
        content: censor_text(
            &trace.content,
            &censored_phrases,
            &config.replace_all_deadnames_with,
        ),
        ..trace
    })
    .collect();

    if results.len() == 0 {
        return None;
    }

    Some(results)
}

pub async fn trace_around_message(
    pool: &Pool<Postgres>,
    config: &Config,
    message_id: &str,
    num_around: i32,
) -> Option<Vec<MessageTrace>> {
    let censored_phrases = fetch_censored_phrases(pool).await;
    let results: Vec<MessageTrace> = query!(
        r#"WITH post_timestamp AS (
            SELECT timestamp_sent, campaign_id
            FROM post
            WHERE id LIKE '%' || $1 || '%'
            ORDER BY timestamp_sent ASC
            LIMIT 1
        ), joined_fields AS (
            SELECT
                post.id,
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
            WHERE
                campaign.id = (SELECT campaign_id FROM post_timestamp)
        )
        SELECT *
        FROM (
            SELECT * FROM joined_fields
            WHERE timestamp_sent <= (SELECT timestamp_sent FROM post_timestamp)
            ORDER BY timestamp_sent DESC
            LIMIT 1 + $2
        )
        UNION
        (
            SELECT * FROM joined_fields
            WHERE timestamp_sent > (SELECT timestamp_sent FROM post_timestamp)
            ORDER BY timestamp_sent ASC
            LIMIT $2
        )
        ORDER BY timestamp_sent ASC"#,
        message_id.trim(),
        num_around as i64
    )
    .fetch_all(pool)
    .await
    .ok()?
    .into_iter()
    .map(|trace_wrapped| MessageTrace {
        id: trace_wrapped.id.unwrap(),
        sender_name: trace_wrapped.sender_name.unwrap(),
        is_censored: trace_wrapped.is_censored.unwrap(),
        player_name: trace_wrapped.player_name.unwrap(),
        campaign_name: trace_wrapped.campaign_name.unwrap(),
        timestamp_sent: trace_wrapped.timestamp_sent.unwrap(),
        timezone_offset: trace_wrapped.timezone_offset.unwrap(),
        content: censor_text(
            &trace_wrapped.content.unwrap(),
            &censored_phrases,
            &config.replace_all_deadnames_with,
        ),
    })
    .collect();

    if results.len() == 0 {
        return None;
    }

    Some(results)
}
