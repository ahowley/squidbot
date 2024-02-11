#[async_std::test]
async fn connect_to_test_db() {
    let pool = data::get_connection_pool("../.env.test").await;
    assert_eq!(pool.options().get_max_connections(), 5);
}
