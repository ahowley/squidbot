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
