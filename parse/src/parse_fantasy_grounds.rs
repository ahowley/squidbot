use std::time::Duration;

use crate::{ChatLog, Post, Roll};
use async_trait::async_trait;
use scraper::{node::Text, ElementRef, Html, Node, Selector};
use sqlx::types::chrono::{DateTime, FixedOffset};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, BufReader, Lines},
};

const DATETIME_STRP: &'static str = "%Y-%m-%d %H:%M %z";
const IGNORE_MESSAGES: [&'static str; 2] = ["Party taking long rest.", "Party taking short rest."];

fn try_get_date_and_time_strings(fragment: &Html) -> Option<[String; 2]> {
    let anchor_selector = Selector::parse("a").unwrap();
    let b_selector = Selector::parse("b").unwrap();
    let anchor_find = fragment.select(&anchor_selector).next();
    let b_find = fragment.select(&b_selector).next();

    if b_find.is_none() {
        return None;
    }

    if anchor_find.is_none() {
        let b_elem = b_find.unwrap();
        let datetime_find = b_elem.text().last();
        if datetime_find.is_none() {
            return None;
        }

        let datetime_str = datetime_find.unwrap();
        if !datetime_str.starts_with("Chat log started at ") {
            return None;
        }
        let datetime_str = datetime_str.replace("Chat log started at ", "");

        if let [date_str, time_str] = datetime_str.split(" / ").collect::<Vec<&str>>()[..] {
            let converted_date_str = date_str
                .split(".")
                .map(|component| {
                    if component.len() < 2 {
                        format!("0{component}")
                    } else {
                        component.to_string()
                    }
                })
                .collect::<Vec<String>>();
            let [day, month, year] = &converted_date_str[..] else {
                return None;
            };

            let converted_time_str = time_str.split(":").collect::<Vec<&str>>();
            let [hours, minutes, ..] = converted_time_str[..] else {
                return None;
            };

            return Some([
                format!("{year}-{month}-{day}"),
                format!("{hours}:{minutes}"),
            ]);
        }

        return None;
    }

    let (anchor_elem, b_elem) = (anchor_find.unwrap(), b_find.unwrap());
    let date_find = anchor_elem.attr("name");
    let time_find = b_elem.text().last();
    if date_find.is_none() || time_find.is_none() {
        return None;
    }

    let (date_str, time_str_raw) = (date_find.unwrap(), time_find.unwrap());
    let time_str_find = time_str_raw.split(" / ").last();
    if time_str_find.is_none() || !time_str_raw.contains("Session started") {
        return None;
    }

    let time_str = time_str_find.unwrap();
    Some([date_str.to_string(), time_str.to_string()])
}

fn get_possible_roll_text<'a>(fragment: &'a Html) -> Option<&'a Text> {
    let possible_text = fragment
        .tree
        .nodes()
        .filter_map(|node_ref| match node_ref.value() {
            Node::Text(text) => Some(text),
            _ => None,
        })
        .last();

    if let Some(text) = possible_text {
        if text.starts_with(" [") && text.ends_with("]") {
            return possible_text;
        }
    }

    None
}

fn try_get_roll_from_possible_roll_text(possible_roll_text: &Text) -> Option<Roll> {
    let mut inside_square_brackets = false;
    let mut possible_roll_string = String::new();
    let mut has_formula = false;
    let mut has_outcome = false;
    let mut is_roll_string = false;
    for symbol in possible_roll_text.chars() {
        if possible_roll_string.len() == 0 && symbol == '[' {
            inside_square_brackets = true;
            continue;
        }

        if symbol == ']' && has_outcome {
            is_roll_string = true;
            break;
        }

        if !inside_square_brackets {
            continue;
        }

        if symbol == 'd' || symbol == 'g' || symbol == 'r' {
            has_formula = true;
            possible_roll_string.push('d');
            continue;
        }

        if symbol == '=' && has_formula {
            has_outcome = true;
        }

        possible_roll_string.push(symbol);
    }

    if is_roll_string {
        if let [formula, outcome] = possible_roll_string.split(" = ").collect::<Vec<&str>>()[..] {
            if let Ok(parsed_outcome) = outcome.parse::<f64>() {
                return Some(Roll {
                    formula: formula.to_string(),
                    outcome: parsed_outcome,
                    single_rolls: vec![],
                });
            }
        }
    }

    None
}

fn try_get_sender_name_and_content_from_font_elem<'a>(
    font_elem: &'a ElementRef<'a>,
) -> Option<[&'a str; 2]> {
    let font_text = font_elem.text().last().unwrap_or("");
    if font_text.chars().next().unwrap_or('[') == '[' {
        return None;
    }

    if let [sender_name, content_raw] = font_text.split(':').collect::<Vec<&str>>()[..] {
        if sender_name.contains("&#62;") {
            return None;
        }

        return Some([sender_name, content_raw]);
    }

    None
}

fn sender_name_and_content_are_valid_message(sender_name: &str, content_raw: &str) -> bool {
    if sender_name.contains("&#62;")
        || sender_name.contains(">")
        || sender_name.contains("Extension")
        || content_raw.contains("&#62;")
        || content_raw.contains(">")
        || (content_raw.starts_with(" [") && !content_raw.starts_with(" [Translation]"))
        || IGNORE_MESSAGES
            .iter()
            .any(|ignored| content_raw.contains(ignored))
    {
        return false;
    }

    true
}

pub struct FantasyGroundsChatLog {
    current_message_id: i64,
    timezone_offset: i32,
    current_message_html: String,
    last_parsed_datetime: Option<DateTime<FixedOffset>>,
    lines: Lines<BufReader<File>>,
}

impl FantasyGroundsChatLog {
    fn try_update_last_parsed_datetime(&mut self, date_str: &str, time_str: &str) {
        let ts_text = format!("{} {} +0000", date_str, time_str);
        if let Ok(timestamp) = DateTime::parse_from_str(&ts_text, DATETIME_STRP) {
            let offset_s = FixedOffset::east_opt(-self.timezone_offset * 3600).unwrap();
            self.last_parsed_datetime = Some(timestamp + offset_s);
        }
    }

    fn increment_when_post_returned(&mut self) {
        self.current_message_id += 1;

        let increment_duration = Duration::new(60, 0);
        let prev_datetime = self.last_parsed_datetime.expect(
            "expected to already have parsed a datetime in fantasy grounds log, but none was found",
        );
        self.last_parsed_datetime = Some(prev_datetime + increment_duration);
    }

    fn post_from_current_message_html(&mut self) -> Option<Post> {
        let fragment = Html::parse_fragment(&self.current_message_html);
        self.current_message_html.drain(..);

        let font_selector = Selector::parse("font").unwrap();
        let font_elem = fragment.select(&font_selector).next();
        if font_elem.is_none() {
            if let Some([date_str, time_str]) = try_get_date_and_time_strings(&fragment) {
                self.try_update_last_parsed_datetime(&date_str, &time_str);
            }

            return None;
        }

        let font_elem = font_elem.unwrap();
        let Some([sender_name, content_raw]) =
            try_get_sender_name_and_content_from_font_elem(&font_elem)
        else {
            return None;
        };

        let mut rolls: Vec<Roll> = vec![];
        if let Some(possible_roll_text) = get_possible_roll_text(&fragment) {
            if let Some(roll) = try_get_roll_from_possible_roll_text(possible_roll_text) {
                rolls.push(roll);
            } else {
                return None;
            }
        }

        let is_message =
            rolls.len() == 0 && sender_name_and_content_are_valid_message(sender_name, content_raw);
        if !is_message {
            return None;
        }

        let post = Post {
            id: self.current_message_id.to_string(),
            sender_name: sender_name.trim().to_string(),
            datetime: self.last_parsed_datetime.clone().unwrap(),
            content_raw: content_raw.trim().to_string(),
            is_message,
            rolls,
        };
        self.increment_when_post_returned();

        Some(post)
    }
}

#[async_trait]
impl ChatLog for FantasyGroundsChatLog {
    async fn new(file: File, timezone_offset: Option<i32>) -> Self {
        let lines = BufReader::new(file).lines();
        let offset = if let Some(hours) = timezone_offset {
            hours
        } else {
            0
        };

        Self {
            current_message_id: 1,
            timezone_offset: offset,
            current_message_html: String::from(""),
            last_parsed_datetime: None,
            lines,
        }
    }

    async fn next_post(&mut self) -> Option<Post> {
        while let Some(line) = self.lines.next_line().await.unwrap() {
            self.current_message_html.push_str(line.as_str());

            if !self.current_message_html.ends_with("<br />") {
                continue;
            }

            if let Some(post) = self.post_from_current_message_html() {
                return Some(post);
            }
        }

        None
    }
}
