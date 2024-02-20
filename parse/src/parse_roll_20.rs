use crate::{ChatLog, Post};
use async_trait::async_trait;
use scraper::{Html, Selector};
use sqlx::types::chrono::{DateTime, FixedOffset, NaiveTime};
use tokio::{
    fs::File,
    io::{AsyncBufReadExt, BufReader, Lines},
};
use unicode_segmentation::UnicodeSegmentation;

const DATETIME_STRP: &'static str = "%B %d, %Y %I:%M%p %z";
const DATE_STRF: &'static str = "%B %d, %Y";
const TIME_STRP_STRF: &'static str = "%I:%M%p %z";
const DIV_OPEN: &'static str = "<div";
const DIV_CLOSE: &'static str = "</div";

pub struct Roll20ChatLog {
    timezone_offset: i32,
    div_depth: i32,
    current_message_html: String,
    last_parsed_sender_name: Option<String>,
    last_parsed_datetime: Option<DateTime<FixedOffset>>,
    lines: Lines<BufReader<File>>,
}

impl Roll20ChatLog {
    async fn post_from_current_message_html(&mut self) -> Option<Post> {
        let fragment = Html::parse_fragment(self.current_message_html.as_str());
        let full_message_selector = Selector::parse(".message").unwrap();
        let sender_selector = Selector::parse(".by").unwrap();
        let timestamp_selector = Selector::parse(".tstamp").unwrap();

        let full_message = fragment.select(&full_message_selector).next()?.value();
        let id = full_message.attr("data-messageid")?;

        if let Some(sender_elem) = fragment.select(&sender_selector).next() {
            let sender_raw = sender_elem.text().collect::<Vec<&str>>().join("");
            self.last_parsed_sender_name = Some(sender_raw.strip_suffix(":").unwrap().to_string());
        }

        if let Some(timestamp_elem) = fragment.select(&timestamp_selector).next() {
            let mut ts_text = timestamp_elem.text().collect::<Vec<&str>>().join("");
            ts_text.push_str(" +0000");

            if let Ok(timestamp) = DateTime::parse_from_str(ts_text.as_str(), DATETIME_STRP) {
                let offset_s = FixedOffset::east_opt(-self.timezone_offset * 3600).unwrap();
                self.last_parsed_datetime = Some(timestamp + offset_s);
            } else if let Ok(_) = NaiveTime::parse_from_str(ts_text.as_str(), TIME_STRP_STRF) {
                let date_prefix = self.last_parsed_datetime.unwrap().format(DATE_STRF);
                let new_ts_text = format!("{date_prefix} {ts_text}");
                let timestamp =
                    DateTime::parse_from_str(new_ts_text.as_str(), DATETIME_STRP).unwrap();
                let offset_s = FixedOffset::east_opt(-self.timezone_offset * 3600).unwrap();
                self.last_parsed_datetime = Some(timestamp + offset_s);
            }
        }

        let datetime = self.last_parsed_datetime.clone().unwrap();

        println!(
            "{id} {} {datetime}",
            self.last_parsed_sender_name.clone().unwrap()
        );

        None
    }
}

#[async_trait]
impl ChatLog for Roll20ChatLog {
    async fn new(file: File, timezone_offset: Option<i32>) -> Self {
        let lines = BufReader::new(file).lines();
        let offset = if let Some(hours) = timezone_offset {
            hours
        } else {
            0
        };

        Roll20ChatLog {
            timezone_offset: offset,
            div_depth: -1,
            current_message_html: String::from(""),
            last_parsed_sender_name: None,
            last_parsed_datetime: None,
            lines,
        }
    }

    async fn next_post(&mut self) -> Option<Post> {
        let mut current_tag = String::from("");
        while let Some(line) = self.lines.next_line().await.unwrap() {
            if self.div_depth == -1 {
                self.div_depth += 1;
                continue;
            }

            self.current_message_html.push_str(line.as_str());

            for grapheme in line.graphemes(true) {
                if current_tag != "" {
                    if grapheme == "<"
                        || (grapheme == ">"
                            && current_tag.graphemes(true).filter(|gr| *gr == "<").count()
                                > current_tag.graphemes(true).filter(|gr| *gr == ">").count() + 1)
                    {
                        current_tag.push_str(grapheme);
                    } else if grapheme == ">" {
                        if current_tag.starts_with(DIV_OPEN) {
                            self.div_depth += 1;
                        } else if current_tag.starts_with(DIV_CLOSE) {
                            self.div_depth -= 1;
                        }
                        current_tag.drain(..);

                        if self.div_depth == -1 {
                            return None;
                        }

                        if self.div_depth == 0 {
                            let post = self.post_from_current_message_html().await;
                            self.current_message_html.drain(..);
                            return post;
                        }
                    } else {
                        current_tag.push_str(grapheme);
                    }
                } else if grapheme == "<" {
                    current_tag.push_str(grapheme);
                }
            }
        }

        None
    }
}
