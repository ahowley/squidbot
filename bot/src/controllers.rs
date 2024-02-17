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
        data::update_posts_from_log(
            pool.clone(),
            campaign_name,
            format!("./chatlogs/{}", campaign_config.log),
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

pub async fn campaign_quote(campaign: Option<String>) -> String {
    let pool = data::create_connection_pool("./.env").await;

    if let Some(campaign_name) = campaign {
        let valid_campaigns = data::fetch_campaign_names(&pool).await;
        if valid_campaigns.contains(&campaign_name) {
            return data::fetch_random_chat_message(&pool, Some(&campaign_name)).await;
        }
    }

    data::fetch_random_chat_message(&pool, None).await
}
