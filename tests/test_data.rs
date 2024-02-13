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
    assert_eq!(inserted_id, pronouns_id);

    let pronouns_values = Pronouns::try_fetch_values(&mut transaction, pronouns_id)
        .await
        .unwrap();
    let new_pronouns = Pronouns::from_values(&pronouns_values).await;
    let new_inserted_id = new_pronouns.fetch_or_insert_id(&mut transaction).await;
    let new_fetched_id = new_pronouns.fetch_or_insert_id(&mut transaction).await;
    transaction.rollback().await.unwrap();

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
    assert_eq!(player.player_name, player_name);

    let mut transaction = data::begin_transaction(&pool).await;
    let inserted_id = player.fetch_or_insert_id(&mut transaction).await;
    let fetched_id = player.fetch_or_insert_id(&mut transaction).await;
    let player_id = player.try_fetch_id(&mut transaction).await.unwrap();
    assert_eq!(inserted_id, fetched_id);
    assert_eq!(inserted_id, player_id);

    let player_values = Player::try_fetch_values(&mut transaction, player_id)
        .await
        .unwrap();
    transaction.rollback().await.unwrap();
    assert_eq!(player_values, player_name);
}

#[async_std::test]
async fn pronouns_map() {
    use data::{Player, Pronouns, PronounsMap};

    let pool = data::create_connection_pool("../.env.test").await;

    let pronouns: Pronouns = "they/them/their/theirs".into();
    let player_name = "Bob".to_string();
    let player = Player::from_values(&player_name).await;

    let mut transaction = data::begin_transaction(&pool).await;
    let pronouns_id = pronouns.fetch_or_insert_id(&mut transaction).await;
    player.fetch_or_insert_id(&mut transaction).await;

    let pronouns_values = Pronouns::try_fetch_values(&mut transaction, pronouns_id)
        .await
        .unwrap();
    let pronouns_map_values = (pronouns_values, player_name.clone());
    let pronouns_map = PronounsMap::from_values(&pronouns_map_values).await;
    let pronouns_map_id = pronouns_map.fetch_or_insert_id(&mut transaction).await;

    let (new_pronouns_values, new_player_name) =
        PronounsMap::try_fetch_values(&mut transaction, pronouns_map_id)
            .await
            .unwrap();
    transaction.rollback().await.unwrap();
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
}
