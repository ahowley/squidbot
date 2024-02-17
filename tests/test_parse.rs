#[async_std::test]
async fn parse_config() {
    let path_to_config = "../test_files/test_config.json".to_string();
    let config = parse::parse_config(path_to_config).await;

    assert_eq!(
        config.campaigns.get("Descent into Avernus").unwrap().log,
        "fnd_test_campaign.db"
    );
}

#[async_std::test]
async fn parse_foundry_chatlog() {
    let path_to_log = "../test_files/fnd_test_campaign.db".to_string();
    let log = parse::parse_log(path_to_log).await;

    let posts: Vec<parse::Post> = log.collect();
    assert_eq!(posts[0].id, "TeStId12345");
    assert_eq!(posts[1].sender_name, "");
    assert_eq!(posts.len(), 4);
}

#[async_std::test]
async fn get_random_message() {
    let message =
        parse::get_random_message("../test_files/test_random_message_templates.json".to_string())
            .await;
    let possible_messages = ["dog\nand then dog".to_string(), "dog ahead".to_string()];

    assert!(possible_messages.contains(&message));
}
