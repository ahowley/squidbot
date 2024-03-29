use async_trait::async_trait;
use parse_config::Config;
pub use parse_dicemath::{
    dicemath, get_roll_from_expression_and_outcomes, num_with_thousands_commas,
};
use parse_fantasy_grounds::FantasyGroundsChatLog;
use parse_foundry::FoundryChatLog;
use parse_random_message_templates::RandomMessageTemplates;
use parse_roll_20::Roll20ChatLog;
use rand::seq::SliceRandom;
use sqlx::types::chrono::{DateTime, FixedOffset};
use std::path::Path;
use tokio::{fs::File, io::AsyncReadExt};
pub use util::*;

pub mod parse_config;
mod parse_dicemath;
mod parse_fantasy_grounds;
mod parse_foundry;
mod parse_random_message_templates;
mod parse_roll_20;
pub mod util;

pub struct RollSingle {
    pub faces: i64,
    pub outcome: i64,
}

pub struct Roll {
    pub formula: String,
    pub outcome: f64,
    pub single_rolls: Vec<RollSingle>,
}

pub struct Post {
    pub id: String,
    pub sender_name: String,
    pub datetime: DateTime<FixedOffset>,
    pub content_raw: String,
    pub is_message: bool,
    pub rolls: Vec<Roll>,
}

#[async_trait]
pub trait ChatLog {
    async fn new(file: File, timezone_offset: Option<i32>) -> Self;

    async fn next_post(&mut self) -> Option<Post>;
}

async fn validate_and_open_file(
    path: &Path,
    starts_with: Option<&str>,
    contains: Option<&str>,
    extension: Option<&str>,
) -> Option<File> {
    let filename = path.file_name().expect("failed to get filename");
    let filename_str = filename.to_str().expect("failed to parse filename to utf8");

    if let Some(prefix) = starts_with {
        if !filename_str.starts_with(prefix) {
            return None;
        }
    };

    if let Some(substring) = contains {
        if !filename_str.contains(substring) {
            return None;
        }
    };

    if let Some(suffix) = extension {
        if path
            .extension()
            .expect("failed to parse file extension to utf8")
            != suffix
        {
            return None;
        }
    };

    Some(File::open(path).await.expect(&format!(
        "couldn't find or failed to open file '{}'",
        filename_str
    )))
}

pub async fn parse_config(path_to_config: String) -> Config {
    let path_to_config = Path::new(&path_to_config);
    let mut file = validate_and_open_file(path_to_config, None, Some("config"), Some("json"))
        .await
        .unwrap();
    let mut config_json = String::new();
    file.read_to_string(&mut config_json)
        .await
        .expect("failed to read contents of config.json");

    Config::parse(&config_json)
        .expect("failed to parse config.json - see README or config.example.json for help")
}

pub async fn parse_foundry_log(path_to_log: &str, timezone_offset: Option<i32>) -> FoundryChatLog {
    let path = Path::new(path_to_log);
    let file = validate_and_open_file(path, Some("fnd_"), None, Some("db"))
        .await
        .expect(
            "wrong filename format for foundry log - see README or config.example.json for help",
        );

    FoundryChatLog::new(file, timezone_offset).await
}

pub async fn parse_roll20_log(path_to_log: &str, timezone_offset: Option<i32>) -> Roll20ChatLog {
    let path = Path::new(path_to_log);
    let file = validate_and_open_file(path, Some("r20_"), None, Some("html"))
        .await
        .expect(
            "wrong filename format for roll20 log - see README or config.example.json for help",
        );

    Roll20ChatLog::new(file, timezone_offset).await
}

pub async fn parse_fantasy_grounds_log(
    path_to_log: &str,
    timezone_offset: Option<i32>,
) -> FantasyGroundsChatLog {
    let path = Path::new(path_to_log);
    let file = validate_and_open_file(path, Some("fg_"), None, Some("html"))
        .await
        .expect(
            "wrong filename format for fantasy grounds log - see README or config.example.json for help",
        );

    FantasyGroundsChatLog::new(file, timezone_offset).await
}

pub async fn get_random_message(path_to_templates: String) -> String {
    let path = Path::new(&path_to_templates);

    let mut file =
        validate_and_open_file(path, None, Some("random_message_templates"), Some("json"))
            .await
            .unwrap();
    let mut templates_json = String::new();
    file.read_to_string(&mut templates_json)
        .await
        .expect("failed to read contents of random_message_templates.json");
    let templates = RandomMessageTemplates::parse(&templates_json)
            .expect("failed to parse random_message_templates.json - see README or random_message_templates.example.json for help");

    if rand::random() {
        let super_template = templates
            .super_templates
            .choose(&mut rand::thread_rng())
            .unwrap();
        let random_words = [
            templates.words.choose(&mut rand::thread_rng()).unwrap(),
            templates.words.choose(&mut rand::thread_rng()).unwrap(),
        ];
        super_template
            .replace("%a", random_words[0])
            .replace("%b", random_words[1])
    } else {
        let template = templates.templates.choose(&mut rand::thread_rng()).unwrap();
        let random_word = templates.words.choose(&mut rand::thread_rng()).unwrap();
        template.replace("%x", random_word)
    }
}
