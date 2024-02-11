use data::{AutoIncrementedId, GeneratedIdTransaction, Pronouns};

#[async_std::test]
async fn connect_to_test_db() {
    let pool = data::get_connection_pool("../.env.test").await;
    assert_eq!(pool.options().get_max_connections(), 5);
}

#[async_std::test]
async fn try_insert_pronouns() {
    let pool = data::get_connection_pool("../.env.test").await;
    let pronouns: Pronouns = "test/pronouns/for/database".into();
    let GeneratedIdTransaction(mut transaction, record_id) =
        pronouns.try_insert(&pool).await.unwrap();

    let retreived_id = Pronouns::fetch_id_by_values(&mut *transaction, &pronouns)
        .await
        .unwrap();

    assert_eq!(record_id, retreived_id);
}
