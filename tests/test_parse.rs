use parse::ChatLog;

#[tokio::test]
async fn parse_config() {
    let path_to_config = "../test_files/test_config.json".to_string();
    let config = parse::parse_config(path_to_config).await;

    assert_eq!(
        config.campaigns.get("Descent into Avernus").unwrap().log,
        "fnd_test_campaign.db"
    );
}

#[tokio::test]
async fn parse_foundry_chatlog() {
    let path_to_log = "../test_files/fnd_test_campaign.db";
    let mut log = parse::parse_foundry_log(path_to_log, None).await;

    let mut posts: Vec<parse::Post> = vec![];
    while let Some(post) = log.next_post().await {
        posts.push(post);
    }
    assert_eq!(posts[0].id, "TeStId12345");
    assert_eq!(posts[1].sender_name, "");
    assert_eq!(posts.len(), 4);
}

#[tokio::test]
async fn get_random_message() {
    let message =
        parse::get_random_message("../test_files/test_random_message_templates.json".to_string())
            .await;
    let possible_messages = ["dog\nand then dog".to_string(), "dog ahead".to_string()];

    assert!(possible_messages.contains(&message));
}
