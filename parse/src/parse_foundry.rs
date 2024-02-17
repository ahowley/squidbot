use crate::{ChatLog, Post, Roll, RollSingle};
use async_trait::async_trait;
use serde::Deserialize;
use sqlx::types::chrono::DateTime;
use std::num::TryFromIntError;
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, BufReader, Lines},
};

#[derive(Deserialize)]
struct Speaker {
    alias: String,
}

#[derive(Deserialize)]
struct RollResult {
    result: i64,
}

#[derive(Deserialize)]
struct RollTerm {
    class: String,
    faces: Option<i64>,
    results: Option<Vec<RollResult>>,
}

#[derive(Deserialize)]
struct RollRaw {
    formula: String,
    terms: Vec<RollTerm>,
    total: f64,
}

impl From<RollRaw> for Roll {
    fn from(roll_raw: RollRaw) -> Self {
        let single_rolls: Vec<RollSingle> = roll_raw
            .terms
            .iter()
            .filter_map(|term| match term.class.as_str() {
                "Die" => Some(
                    term.results
                        .as_ref()
                        .unwrap()
                        .iter()
                        .map(|res| RollSingle {
                            faces: term.faces.unwrap(),
                            outcome: res.result,
                        })
                        .collect::<Vec<_>>(),
                ),
                _ => None,
            })
            .flatten()
            .collect();

        Roll {
            formula: roll_raw.formula,
            outcome: roll_raw.total,
            single_rolls,
        }
    }
}

impl FromIterator<RollRaw> for Vec<Roll> {
    fn from_iter<T: IntoIterator<Item = RollRaw>>(iter: T) -> Self {
        iter.into_iter()
            .map(|roll_raw| Roll::from(roll_raw))
            .collect()
    }
}

#[derive(Deserialize)]
struct PostRaw {
    _id: String,
    #[serde(alias = "type")]
    type_number: u8,
    speaker: Speaker,
    timestamp: i64,
    content: String,
    whisper: Vec<String>,
    rolls: Vec<String>,
}

impl PostRaw {
    fn parse(line: &str) -> serde_json::Result<Self> {
        serde_json::from_str(line)
    }

    fn contains_rolls(&self) -> bool {
        self.rolls.len() > 0
    }

    fn parse_rolls(&self) -> Vec<Roll> {
        if !self.contains_rolls() {
            panic!(
                "post with id {} doesn't contain any rolls, but parsing rolls was attempted",
                self._id
            );
        }

        self.rolls
            .iter()
            .map(|roll| serde_json::from_str::<RollRaw>(roll).unwrap())
            .collect()
    }
}

impl From<PostRaw> for Post {
    fn from(foundry_post: PostRaw) -> Self {
        let timestamp_s = foundry_post.timestamp / 1000;
        let timestamp_ns: Result<u32, TryFromIntError> =
            ((foundry_post.timestamp as i64 % 1000) * 1_000_000).try_into();
        let timestamp_ns = timestamp_ns.unwrap();

        let is_message = !foundry_post.contains_rolls();

        let rolls: Vec<Roll> = if foundry_post.contains_rolls() {
            foundry_post.parse_rolls()
        } else {
            vec![]
        };

        Self {
            id: foundry_post._id,
            sender_name: foundry_post.speaker.alias,
            datetime: DateTime::from_timestamp(timestamp_s, timestamp_ns)
                .unwrap()
                .into(),
            is_message,
            content_raw: foundry_post.content,
            rolls,
        }
    }
}

pub struct FoundryChatLog {
    lines: Lines<BufReader<File>>,
}

#[async_trait]
impl ChatLog for FoundryChatLog {
    async fn new(file: File) -> Self {
        let lines = BufReader::new(file).lines();

        FoundryChatLog { lines }
    }

    async fn next_post(&mut self) -> Option<Post> {
        while let Some(line) = self.lines.next_line().await.unwrap() {
            let post = match PostRaw::parse(&line) {
                Ok(res) => res,
                Err(_) => continue,
            };

            if post.type_number == 0 || post.whisper.len() > 0 {
                continue;
            }

            return Some(post.into());
        }

        None
    }
}
