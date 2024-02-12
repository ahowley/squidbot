use data::{GeneratedIdTransaction, Player, Pronouns, ShapeInterface};

#[async_std::test]
async fn connect_to_test_db() {
    let pool = data::get_connection_pool("../.env.test").await;
    assert_eq!(pool.options().get_max_connections(), 5);
}

#[async_std::test]
async fn pronouns_interface() {
    let pool = data::get_connection_pool("../.env.test").await;

    let pronouns: Pronouns = "test/pronouns/for/database".into();
    let GeneratedIdTransaction(mut transaction, record_id) =
        pronouns.try_insert(&pool).await.unwrap();
    let retreived_id = Pronouns::fetch_id_by_values(&mut *transaction, &pronouns)
        .await
        .unwrap();
    assert_eq!(record_id, retreived_id);

    let retrieved_pronouns = Pronouns::fetch_values(&mut *transaction, record_id)
        .await
        .unwrap();
    assert_eq!(retrieved_pronouns[0], pronouns.subj);

    let new_pronouns = Pronouns::from_values(&retrieved_pronouns).await;
    assert_eq!(new_pronouns.subj, pronouns.subj);
}

#[async_std::test]
async fn player_table() {
    let pool = data::get_connection_pool("../.env.test").await;

    let player_values = "Bob".to_string();
    let player = Player::from_values(&player_values).await;
    let GeneratedIdTransaction(mut transaction, record_id) =
        player.try_insert(&pool).await.unwrap();
    let retreived_id = Player::fetch_id_by_values(&mut *transaction, &player)
        .await
        .unwrap();
    assert_eq!(record_id, retreived_id);

    let retrieved_player = Player::fetch_values(&mut *transaction, record_id)
        .await
        .unwrap();
    assert_eq!(player.player_name, retrieved_player);

    let new_player = Player::from_values(&retrieved_player).await;
    assert_eq!(new_player.player_name, player.player_name);
}
