#[test]
fn parse_foundry_chatlog() {
    let path_to_log = "../test_files/fnd_test_campaign.db";
    let mut log = parse::parseLog(path_to_log);

    let first_post = log.next().unwrap();
    assert_eq!(first_post.id(), "TeStId12345");
}
