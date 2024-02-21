use rayon::iter::{IntoParallelIterator, ParallelIterator};
use std::sync::Arc;

pub async fn message() -> String {
    parse::get_random_message("./random_message_templates.json".to_string()).await
}

pub async fn update_chatlogs() {
    let pool = Arc::new(data::create_connection_pool("./.env").await);
    let config = parse::parse_config("./config.json".to_string()).await;

    let mut transaction = data::begin_transaction(&pool).await;
    data::update_players(&mut transaction, &config).await;
    data::update_campaigns(&mut transaction, &config).await;
    transaction
        .commit()
        .await
        .expect("failed to commit transaction");

    for (campaign_name, campaign_config) in &config.campaigns {
        // TODO: refactor ChatLogs to bake offset into db to avoid the need to check this so much
        let offset = if campaign_config.log.starts_with("fnd_") {
            None
        } else {
            Some(campaign_config.timezone_offset)
        };

        println!("Updating campaign: {campaign_name}");
        data::update_posts_from_log(
            &pool,
            campaign_name,
            "./chatlogs",
            campaign_config.log.as_str(),
            offset,
        )
        .await;
    }
}

pub async fn dump_unmapped_senders() -> String {
    let config = parse::parse_config("./config.json".to_string()).await;
    let senders_map = data::dump_unmapped_senders(&config).await;
    let mut message = "```".to_string();

    for (campaign, senders) in senders_map {
        message.push_str(&format!("--------{campaign}--------\n"));
        for sender in senders {
            message.push_str(&format!("\"{sender}\",\n"));
        }
    }
    message.push_str("```");

    message
}

pub async fn campaigns() -> Vec<String> {
    let pool = data::create_connection_pool("./.env").await;
    data::fetch_campaign_names(&pool).await
}

pub async fn senders() -> Vec<String> {
    let pool = data::create_connection_pool("./.env").await;
    data::fetch_sender_names(&pool).await
}

pub async fn players() -> Vec<String> {
    let pool = data::create_connection_pool("./.env").await;
    data::fetch_player_names(&pool).await
}

pub async fn campaign_quote(
    campaign: Option<String>,
    sender: Option<String>,
    player: Option<String>,
) -> String {
    let pool = data::create_connection_pool("./.env").await;
    let config = parse::parse_config("./config.json".to_string()).await;

    let campaign_name = campaign.unwrap_or("".to_string());
    let sender_name = sender.unwrap_or("".to_string());
    let player_name = player.unwrap_or("".to_string());

    let result =
        data::fetch_random_chat_message(&pool, &config, &campaign_name, &sender_name, &player_name)
            .await;

    if result.len() > 0 {
        result
    } else {
        "Sorry, I couldn't find any quotes with these filters!".to_string()
    }
}

pub async fn who_sent(message: String) -> String {
    let pool = data::create_connection_pool("./.env").await;

    if let Some(results) = data::trace_message(&pool, &message).await {
        let mut response = format!("Here's everyone who sent '`{}`':\n\n", message);
        response.push_str(
            &results
                .into_iter()
                .map(|trace| format!("```{trace}```"))
                .collect::<Vec<String>>()
                .join("\n"),
        );

        return response;
    }

    return format!("Sorry - I couldn't find '`{}`' in the database!", message);
}

pub async fn around(message_id: String, num_around: i32) -> String {
    let pool = data::create_connection_pool("./.env").await;
    let config = parse::parse_config("./config.json".to_string()).await;

    if let Some(results) =
        data::trace_around_message(&pool, &config, message_id.as_str(), num_around).await
    {
        let mut response = format!("Here's the context:\n\n");
        response.push_str(
            &results
                .into_iter()
                .map(|trace| format!("```{trace}```"))
                .collect::<Vec<String>>()
                .join("\n"),
        );

        return response;
    }

    return format!(
        "Sorry - I couldn't find '`{}`' in the database!",
        message_id
    );
}

pub async fn roll(expr: &str) -> String {
    let result = parse::dicemath(expr);

    match result {
        Some(value) => format!("Result for `{expr}`:\n`{value}`"),
        None => format!("Sorry, I couldn't figure out how to parse `{expr}`!"),
    }
}

pub async fn odds(expr: &str, val: f64, num_rolls: u64) -> String {
    let results: Vec<bool> = (0..num_rolls)
        .into_par_iter()
        .filter_map(|_| {
            let result = parse::dicemath(expr)?;
            if result >= val {
                Some(true)
            } else {
                None
            }
        })
        .collect();

    format!(
        "Out of `{}` rolls of `{expr}`, I rolled a `{val}` or higher a total of `{}` times.\n\nMy estimated odds of rolling a `{val}` or higher is about `{}`%!",
        parse::num_with_thousands_commas(num_rolls as u64),
        parse::num_with_thousands_commas(results.len() as u64),
        results.len() as f64 / (num_rolls as f64) * 100.
    )
}
