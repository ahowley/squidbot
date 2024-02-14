use data::{IdInterface, ShapeInterface};

#[async_std::test]
async fn connect_to_test_db() {
    let pool = data::create_connection_pool("../.env.test").await;
    assert_eq!(pool.options().get_max_connections(), 5);
}

#[async_std::test]
async fn pronouns() {
    use data::Pronouns;

    let pool = data::create_connection_pool("../.env.test").await;
    let pronouns: Pronouns = "they/them/their/theirs".into();

    let mut transaction = data::begin_transaction(&pool).await;
    let inserted_id = pronouns.fetch_or_insert_id(&mut transaction).await;
    let pronouns_id = pronouns.try_fetch_id(&mut transaction).await.unwrap();

    let pronouns_values = Pronouns::try_fetch_values(&mut transaction, pronouns_id)
        .await
        .unwrap();
    let new_pronouns = Pronouns::from_values(&pronouns_values).await;
    let new_inserted_id = new_pronouns.fetch_or_insert_id(&mut transaction).await;
    let new_fetched_id = new_pronouns.fetch_or_insert_id(&mut transaction).await;
    transaction.rollback().await.unwrap();

    assert_eq!(inserted_id, pronouns_id);
    assert_eq!(new_pronouns.subj, pronouns.subj);
    assert_eq!(new_pronouns.obj, pronouns.obj);
    assert_eq!(new_pronouns.poss_pres, pronouns.poss_pres);
    assert_eq!(new_pronouns.poss_past, pronouns.poss_past);
    assert_eq!(new_inserted_id, new_fetched_id);
}

#[async_std::test]
async fn player() {
    use data::Player;

    let pool = data::create_connection_pool("../.env.test").await;
    let player_name = "Bob".to_string();
    let player: Player = Player::from_values(&player_name).await;

    let mut transaction = data::begin_transaction(&pool).await;
    let inserted_id = player.fetch_or_insert_id(&mut transaction).await;
    let fetched_id = player.fetch_or_insert_id(&mut transaction).await;
    let player_id = player.try_fetch_id(&mut transaction).await.unwrap();

    let player_values = Player::try_fetch_values(&mut transaction, player_id)
        .await
        .unwrap();

    transaction.rollback().await.unwrap();
    assert_eq!(player.player_name, player_name);
    assert_eq!(inserted_id, fetched_id);
    assert_eq!(inserted_id, player_id);
    assert_eq!(player_values, player_name);
}

#[async_std::test]
async fn pronouns_map_and_censor() {
    use data::{Censor, Player, Pronouns, PronounsMap};

    let pool = data::create_connection_pool("../.env.test").await;

    let pronouns: Pronouns = "they/them/their/theirs".into();
    let player_name = "Bob".to_string();
    let player = Player::from_values(&player_name).await;

    let mut transaction = data::begin_transaction(&pool).await;
    let pronouns_id = pronouns.fetch_or_insert_id(&mut transaction).await;
    let dupe_pronouns_id = pronouns.fetch_or_insert_id(&mut transaction).await;
    let player_id = player.fetch_or_insert_id(&mut transaction).await;
    let dupe_player_id = player.fetch_or_insert_id(&mut transaction).await;

    let pronouns_map_values = [pronouns_id, player_id];
    let pronouns_map = PronounsMap::from_values(&pronouns_map_values).await;
    let pronouns_map_id = pronouns_map.fetch_or_insert_id(&mut transaction).await;
    let dupe_pronouns_map_id = pronouns_map.fetch_or_insert_id(&mut transaction).await;

    let [new_pronouns_id, new_player_id] =
        PronounsMap::try_fetch_values(&mut transaction, pronouns_map_id)
            .await
            .unwrap();

    let censor_values = ["Test Deadname".to_string(), player_name.clone()];
    let censor = Censor::from_values(&censor_values).await;
    let censor_id = censor.fetch_or_insert_id(&mut transaction).await;
    let dupe_censor_id = censor.fetch_or_insert_id(&mut transaction).await;
    let [avoid_text, censor_player_name] = Censor::try_fetch_values(&mut transaction, censor_id)
        .await
        .unwrap();

    transaction.rollback().await.unwrap();
    assert_eq!(pronouns_id, dupe_pronouns_id);
    assert_eq!(player_id, dupe_player_id);
    assert_eq!(pronouns_map_id, dupe_pronouns_map_id);
    assert_eq!(censor_id, dupe_censor_id);

    assert_eq!(new_player_id, player_id);
    assert_eq!(new_pronouns_id, pronouns_id);

    assert_eq!(censor_player_name, player_name);
    assert_eq!(avoid_text, censor_values[0]);
}

#[async_std::test]
async fn campaign() {
    use data::{Campaign, Player};

    let pool = data::create_connection_pool("../.env.test").await;
    let mut transaction = data::begin_transaction(&pool).await;

    let player_name = "Bob".to_string();
    let player = Player::from_values(&player_name).await;
    player.fetch_or_insert_id(&mut transaction).await;

    let campaign_values = ("Curse of Strahd".to_string(), player_name.clone(), -6);
    let campaign = Campaign::from_values(&campaign_values).await;
    let campaign_id = campaign.fetch_or_insert_id(&mut transaction).await;
    let (campaign_name, dm_name, timezone_offset) =
        Campaign::try_fetch_values(&mut transaction, campaign_id)
            .await
            .unwrap();

    transaction.rollback().await.unwrap();
    assert_eq!(dm_name, player_name);
    assert_eq!(campaign_name, campaign.campaign_name);
    assert_eq!(campaign_name, campaign_values.0);
    assert_eq!(timezone_offset, -6);
}

#[async_std::test]
async fn update_players() {
    let pool = data::create_connection_pool("../.env.test").await;
    let mut transaction = data::begin_transaction(&pool).await;

    let config = parse::parse_config("../test_files/test_config.json".to_string()).await;
    data::update_players(&mut transaction, &config).await;

    let bob_pronouns = sqlx::query!(
        r#"SELECT subj, obj, poss_pres, poss_past
        FROM pronouns_map
            JOIN pronouns ON pronouns_id = pronouns.id
            JOIN player ON player_id = player.id
        WHERE
            player_name = $1"#,
        "Bob"
    )
    .fetch_one(&mut *transaction)
    .await
    .unwrap();

    let bob_deadnames = sqlx::query!(
        r#"SELECT avoid_text
        FROM censor
            JOIN player ON player_id = player.id
        WHERE
            player_name = $1"#,
        "Bob"
    )
    .fetch_one(&mut *transaction)
    .await
    .unwrap();

    let alex_pronouns = sqlx::query!(
        r#"SELECT pronouns_id FROM pronouns_map
            JOIN player ON player_id = player.id
        WHERE
            player_name = $1"#,
        "Alex",
    )
    .fetch_all(&mut *transaction)
    .await
    .unwrap();

    assert_eq!(bob_pronouns.subj, "he");
    assert_eq!(bob_pronouns.obj, "him");
    assert_eq!(bob_pronouns.poss_pres, "his");
    assert_eq!(bob_pronouns.poss_past, "his");
    assert_eq!(bob_deadnames.avoid_text, "Bobby");
    assert_eq!(alex_pronouns.len(), 2);

    let config = parse::parse_config("../test_files/test_config_update.json".to_string()).await;
    data::update_players(&mut transaction, &config).await;

    let bob_pronouns = sqlx::query!(
        r#"SELECT subj, obj, poss_pres, poss_past
        FROM pronouns_map
            JOIN pronouns ON pronouns_id = pronouns.id
            JOIN player ON player_id = player.id
        WHERE
            player_name = $1"#,
        "Bob"
    )
    .fetch_one(&mut *transaction)
    .await
    .unwrap();

    let bob_deadnames = sqlx::query!(
        r#"SELECT avoid_text
        FROM censor
            JOIN player ON player_id = player.id
        WHERE
            player_name = $1"#,
        "Bob"
    )
    .fetch_one(&mut *transaction)
    .await;

    let alex_pronouns = sqlx::query!(
        r#"SELECT pronouns_id FROM pronouns_map
            JOIN player ON player_id = player.id
        WHERE
            player_name = $1"#,
        "Alex",
    )
    .fetch_all(&mut *transaction)
    .await
    .unwrap();

    assert_eq!(bob_pronouns.subj, "they");
    assert_eq!(bob_pronouns.obj, "them");
    assert_eq!(bob_pronouns.poss_pres, "their");
    assert_eq!(bob_pronouns.poss_past, "theirs");
    assert_eq!(
        match bob_deadnames {
            Err(_) => 1,
            _ => 0,
        },
        1
    );
    assert_eq!(alex_pronouns.len(), 1);

    transaction.rollback().await.unwrap();
}
