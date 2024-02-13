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

    let pronouns_values = Pronouns::try_fetch_values(&mut transaction, pronouns_id)
        .await
        .unwrap();
    let pronouns_map_values = (pronouns_values, player_name.clone());
    let pronouns_map = PronounsMap::from_values(&pronouns_map_values).await;
    let pronouns_map_id = pronouns_map.fetch_or_insert_id(&mut transaction).await;
    let dupe_pronouns_map_id = pronouns_map.fetch_or_insert_id(&mut transaction).await;

    let (new_pronouns_values, new_player_name) =
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

    assert_eq!(new_player_name, player.player_name);
    assert_eq!(
        new_pronouns_values,
        [
            pronouns.subj,
            pronouns.obj,
            pronouns.poss_pres,
            pronouns.poss_past
        ]
    );

    assert_eq!(censor_player_name, new_player_name);
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

    let campaign_values = ["Curse of Strahd".to_string(), player_name.clone()];
    let campaign = Campaign::from_values(&campaign_values).await;
    let campaign_id = campaign.fetch_or_insert_id(&mut transaction).await;
    let [campaign_name, dm_name] = Campaign::try_fetch_values(&mut transaction, campaign_id)
        .await
        .unwrap();

    transaction.rollback().await.unwrap();
    assert_eq!(dm_name, player_name);
    assert_eq!(campaign_name, campaign.campaign_name);
    assert_eq!(campaign_name, campaign_values[0]);
}
