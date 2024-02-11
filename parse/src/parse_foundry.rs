use crate::{ChatLog, Post, Roll, RollSingle};
use serde::Deserialize;
use sqlx::types::chrono::DateTime;
use std::{
    io::{BufRead, BufReader, Lines, Read},
    num::TryFromIntError,
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

pub struct FoundryChatLog<F: Read> {
    lines: Lines<BufReader<F>>,
}

impl<F: Read> Iterator for FoundryChatLog<F> {
    type Item = Post;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let line = match self.lines.next() {
                Some(res) => res.unwrap(),
                None => return None,
            };

            let post = match PostRaw::parse(&line) {
                Ok(res) => res,
                Err(_) => continue,
            };

            if post.type_number == 0 || post.whisper.len() > 0 {
                continue;
            }

            return Some(post.into());
        }
    }
}

impl<F: Read> ChatLog<F> for FoundryChatLog<F> {
    fn new(file: F) -> Self {
        let lines = BufReader::new(file).lines();

        FoundryChatLog { lines }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_message() {
        let mock_db_file_contents = r#"{"type":2,"user":"TeStId12345","timestamp":1659763066302,"flavor":"","content":"foobar!","speaker":{"scene":"TeStId12345","token":"TeStId12345","actor":"TeStId12345","alias":"cool_guy 421"},"whisper":[],"blind":false,"emote":false,"flags":{"polyglot":{"language":"common"}},"_id":"TeStId12345","rolls":[],"sound":null}
"#;
        let mock_db_bytes = mock_db_file_contents.as_bytes();
        let mut log = FoundryChatLog::new(mock_db_bytes);
        let first_post = log.next().unwrap();

        assert_eq!(first_post.id, "TeStId12345");
        assert_eq!(first_post.sender_name, "cool_guy 421");
        assert_eq!(
            first_post.datetime.timestamp_nanos_opt().unwrap(),
            1659763066302000000
        );
        assert_eq!(first_post.content_raw, "foobar!");
        assert_eq!(first_post.is_message, true);
        assert_eq!(first_post.rolls.len(), 0);
    }

    #[test]
    fn num_posts() {
        let mock_db_file_contents = r#"{"type":2,"user":"TeStId12345","timestamp":1659763066302,"flavor":"","content":"foobar!","speaker":{"scene":"TeStId12345","token":"TeStId12345","actor":"TeStId12345","alias":"cool_guy 421"},"whisper":[],"blind":false,"emote":false,"flags":{"polyglot":{"language":"common"}},"_id":"TeStId12345","rolls":[],"sound":null}
{"type":2,"user":"TeStId12345","timestamp":1659763066302,"flavor":"","content":"foobar!","speaker":{"scene":"TeStId12345","token":"TeStId12345","actor":"TeStId12345","alias":"cool_guy 421"},"whisper":[],"blind":false,"emote":false,"flags":{"polyglot":{"language":"common"}},"_id":"TeStId12345","rolls":[],"sound":null}
"#;
        let mock_db_bytes = mock_db_file_contents.as_bytes();
        let log = FoundryChatLog::new(mock_db_bytes);

        let mut num_posts = 0;
        for _ in log {
            num_posts += 1;
        }

        assert_eq!(num_posts, 2);
    }

    #[test]
    fn ignore_whispers() {
        let mock_db_file_contents = r#"{"type":2,"user":"TeStId12345","timestamp":1659763066302,"flavor":"","content":"foobar!","speaker":{"scene":"TeStId12345","token":"TeStId12345","actor":"TeStId12345","alias":"cool_guy 421"},"whisper":[],"blind":false,"emote":false,"flags":{"polyglot":{"language":"common"}},"_id":"TeStId12345","rolls":[],"sound":null}
{"type":2,"user":"TeStId12345","timestamp":1659763066302,"flavor":"","content":"foobar!","speaker":{"scene":"TeStId12345","token":"TeStId12345","actor":"TeStId12345","alias":"cool_guy 421"},"whisper":["TeStId12345"],"blind":false,"emote":false,"flags":{"polyglot":{"language":"common"}},"_id":"TeStId12345","rolls":[],"sound":null}
"#;
        let mock_db_bytes = mock_db_file_contents.as_bytes();
        let log = FoundryChatLog::new(mock_db_bytes);

        let mut num_posts = 0;
        for _ in log {
            num_posts += 1;
        }

        assert_eq!(num_posts, 1);
    }

    #[test]
    fn ignore_item_cards() {
        let mock_db_file_contents = r#"{"type":2,"user":"TeStId12345","timestamp":1659763066302,"flavor":"","content":"foobar!","speaker":{"scene":"TeStId12345","token":"TeStId12345","actor":"TeStId12345","alias":"cool_guy 421"},"whisper":[],"blind":false,"emote":false,"flags":{"polyglot":{"language":"common"}},"_id":"TeStId12345","rolls":[],"sound":null}
{"type":0,"user":"TeStId12345","timestamp":1659763066302,"flavor":"","content":"foobar!","speaker":{"scene":"TeStId12345","token":"TeStId12345","actor":"TeStId12345","alias":"cool_guy 421"},"whisper":[],"blind":false,"emote":false,"flags":{"polyglot":{"language":"common"}},"_id":"TeStId12345","rolls":[],"sound":null}
"#;
        let mock_db_bytes = mock_db_file_contents.as_bytes();
        let log = FoundryChatLog::new(mock_db_bytes);

        let mut num_posts = 0;
        for _ in log {
            num_posts += 1;
        }

        assert_eq!(num_posts, 1);
    }

    #[test]
    fn parse_roll() {
        let mock_db_file_contents = r#"{"user":"TeStId12345","type":5,"content":"12","sound":"sounds/dice.wav","speaker":{"scene":"TeStId12345","token":"TeStId12345","actor":"TeStId12345","alias":"cool_guy 421"},"flags":{"dnd5e":{"roll":{"type":"skill","skillId":"ste"}}},"flavor":"Stealth Skill Check (Dexterity) (Disadvantage)","rolls":["{\"class\":\"D20Roll\",\"options\":{\"flavor\":\"Stealth Skill Check (Dexterity)\",\"advantageMode\":-1,\"defaultRollMode\":\"publicroll\",\"rollMode\":\"publicroll\",\"critical\":20,\"fumble\":1,\"reliableTalent\":false,\"configured\":true},\"dice\":[],\"formula\":\"2d20kl + 0 + 0\",\"terms\":[{\"class\":\"Die\",\"options\":{\"critical\":20,\"fumble\":1,\"disadvantage\":true},\"evaluated\":true,\"number\":2,\"faces\":20,\"modifiers\":[\"kl\"],\"results\":[{\"result\":12,\"active\":true},{\"result\":20,\"active\":false,\"discarded\":true}]},{\"class\":\"OperatorTerm\",\"options\":{},\"evaluated\":true,\"operator\":\"+\"},{\"class\":\"NumericTerm\",\"options\":{},\"evaluated\":true,\"number\":0},{\"class\":\"OperatorTerm\",\"options\":{},\"evaluated\":true,\"operator\":\"+\"},{\"class\":\"NumericTerm\",\"options\":{},\"evaluated\":true,\"number\":0}],\"total\":12,\"evaluated\":true}"],"timestamp":1667020040507,"whisper":[],"blind":false,"emote":false,"_id":"TeStId12345"}
"#;
        let mock_db_bytes = mock_db_file_contents.as_bytes();
        let mut log = FoundryChatLog::new(mock_db_bytes);

        let first_post = log.next().unwrap();
        let rolls = first_post.rolls;

        assert_eq!(rolls.len(), 1);
        assert_eq!(rolls[0].formula, "2d20kl + 0 + 0");
        assert_eq!(rolls[0].single_rolls.len(), 2);
        assert_eq!(rolls[0].outcome, 12.);

        assert_eq!(rolls[0].single_rolls[0].faces, 20);
        assert_eq!(rolls[0].single_rolls[0].outcome, 12);
        assert_eq!(rolls[0].single_rolls[1].outcome, 20);
    }

    #[test]
    fn parse_multi_rolls() {
        let mock_db_file_contents = r#"{"user":"TeStId12345","type":5,"content":"18","sound":"sounds/dice.wav","speaker":{"scene":"TeStId12345","token":"TeStId12345","actor":"TeStId12345","alias":"cool_guy 421"},"flags":{"dnd5e":{"roll":{"type":"skill","skillId":"ste"}}},"flavor":"Stealth Skill Check (Dexterity) (Disadvantage)","rolls":["{\"class\":\"D20Roll\",\"options\":{\"flavor\":\"Stealth Skill Check (Dexterity)\",\"advantageMode\":-1,\"defaultRollMode\":\"publicroll\",\"rollMode\":\"publicroll\",\"critical\":20,\"fumble\":1,\"reliableTalent\":false,\"configured\":true},\"dice\":[],\"formula\":\"3d6 + 2d8 + 3\",\"terms\":[{\"class\":\"Die\",\"options\":{\"critical\":20,\"fumble\":1,\"disadvantage\":true},\"evaluated\":true,\"number\":3,\"faces\":6,\"modifiers\":[],\"results\":[{\"result\":3,\"active\":true},{\"result\":6,\"active\":true},{\"result\":5,\"active\":true}]},{\"class\":\"OperatorTerm\",\"options\":{},\"evaluated\":true,\"operator\":\"+\"},{\"class\":\"Die\",\"options\":{\"critical\":20,\"fumble\":1,\"disadvantage\":true},\"evaluated\":true,\"number\":2,\"faces\":8,\"modifiers\":[],\"results\":[{\"result\":7,\"active\":true},{\"result\":8,\"active\":true}]},{\"class\":\"OperatorTerm\",\"options\":{},\"evaluated\":true,\"operator\":\"+\"},{\"class\":\"NumericTerm\",\"options\":{},\"evaluated\":true,\"number\":3}],\"total\":18,\"evaluated\":true}"],"timestamp":1661568249676,"whisper":[],"blind":false,"emote":false,"_id":"TeStId12356"}
"#;
        let mock_db_bytes = mock_db_file_contents.as_bytes();
        let mut log = FoundryChatLog::new(mock_db_bytes);

        let first_post = log.next().unwrap();
        let rolls = first_post.rolls;

        assert_eq!(rolls.len(), 1);
        assert_eq!(rolls[0].formula, "3d6 + 2d8 + 3");
        assert_eq!(rolls[0].single_rolls.len(), 5);
        assert_eq!(rolls[0].outcome, 18.);

        assert_eq!(rolls[0].single_rolls[0].faces, 6);
        assert_eq!(rolls[0].single_rolls[0].outcome, 3);
        assert_eq!(rolls[0].single_rolls[1].outcome, 6);
        assert_eq!(rolls[0].single_rolls[2].outcome, 5);
        assert_eq!(rolls[0].single_rolls[3].faces, 8);
        assert_eq!(rolls[0].single_rolls[3].outcome, 7);
        assert_eq!(rolls[0].single_rolls[4].outcome, 8);
    }
}
