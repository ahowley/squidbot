use parse_config::Config;
use parse_foundry::FoundryChatLog;
use sqlx::types::chrono::{DateTime, FixedOffset};
use std::{fs::File, io::Read, path::Path};

mod parse_config;
mod parse_foundry;

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

pub trait ChatLog<F: Read>: Iterator<Item = Post> {
    fn new(file: F) -> Self
    where
        Self: Sized;
}

fn validate_and_open_file(
    path: &Path,
    starts_with: Option<&str>,
    contains: Option<&str>,
    extension: Option<&str>,
) -> File {
    let filename = path.file_name().expect("failed to get filename");
    let filename_str = filename.to_str().expect("failed to parse filename to utf8");

    if let Some(prefix) = starts_with {
        if !filename_str.starts_with(prefix) {
            panic!(
                "filename '{}' doesn't start with the expected prefix '{}'",
                filename_str, prefix
            );
        }
    };

    if let Some(substring) = contains {
        if !filename_str.contains(substring) {
            panic!(
                "filename '{}' doesn't contain expected substring '{}'",
                filename_str, substring
            );
        }
    };

    if let Some(suffix) = extension {
        if path
            .extension()
            .expect("failed to parse file extension to utf8")
            != suffix
        {
            panic!(
                "filename '{}' doesn't end with expected file extension '{}'",
                filename_str, suffix
            );
        }
    };

    File::open(path).expect(&format!(
        "couldn't find or failed to open file '{}'",
        filename_str
    ))
}

pub fn parse_config(path_to_config: &str) -> Config {
    let path_to_config = Path::new(path_to_config);
    let mut file = validate_and_open_file(path_to_config, None, Some("config"), Some("json"));
    let mut config_json = String::new();
    file.read_to_string(&mut config_json)
        .expect("failed to read contents of config.json");

    Config::parse(&config_json)
        .expect("failed to parse config.json - see README or config.example.json for help")
}

pub fn parse_log(path_to_log: &str) -> Box<dyn ChatLog<File>> {
    let path = Path::new(path_to_log);

    let file = validate_and_open_file(path, Some("fnd_"), None, Some("db"));
    let log = FoundryChatLog::new(file);

    Box::new(log)
}
