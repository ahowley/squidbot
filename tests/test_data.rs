use data::{GeneratedIdTransaction, Pronouns, ShapeInterface};

#[async_std::test]
async fn connect_to_test_db() {
    let pool = data::get_connection_pool("../.env.test").await;
    assert_eq!(pool.options().get_max_connections(), 5);
}

#[async_std::test]
async fn try_insert_autoincrementing() {
    let pool = data::get_connection_pool("../.env.test").await;
    let pronouns: Pronouns = "test/pronouns/for/database".into();
    let GeneratedIdTransaction(mut transaction, record_id) =
        pronouns.try_insert(&pool).await.unwrap();

    let retreived_id = Pronouns::fetch_id_by_values(&mut *transaction, &pronouns)
        .await
        .unwrap();

    assert_eq!(record_id, retreived_id);
}

#[async_std::test]
async fn fetch_values_autoincrementing() {
    let pool = data::get_connection_pool("../.env.test").await;
    let pronouns: Pronouns = "test/pronouns/for/database".into();
    let GeneratedIdTransaction(mut transaction, record_id) =
        pronouns.try_insert(&pool).await.unwrap();

    let retrieved_pronouns = Pronouns::fetch_values(&mut *transaction, record_id)
        .await
        .unwrap();

    assert_eq!(retrieved_pronouns[0], pronouns.subj);
}

#[async_std::test]
async fn from_values_autoincrementing() {
    let pool = data::get_connection_pool("../.env.test").await;
    let pronouns: Pronouns = "test/pronouns/for/database".into();
    let GeneratedIdTransaction(mut transaction, record_id) =
        pronouns.try_insert(&pool).await.unwrap();
    let retrieved_pronouns = Pronouns::fetch_values(&mut *transaction, record_id)
        .await
        .unwrap();
    let new_pronouns = Pronouns::from_values(&retrieved_pronouns).await;

    assert_eq!(new_pronouns.subj, pronouns.subj);
}
