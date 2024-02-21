use crate::{get_roll_from_expression_and_outcomes, ChatLog, Post, Roll};
use async_trait::async_trait;
use scraper::{html::Select, ElementRef, Html, Selector};
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

fn trim_whitespace(s: &str) -> String {
    let mut new_str = s.trim().to_owned();
    let mut prev = ' '; // The initial value doesn't really matter
    new_str.retain(|ch| {
        let result = ch != ' ' || prev != ' ';
        prev = ch;
        result
    });
    new_str
}

fn try_get_roll_from_plain(
    formula_elem: &ElementRef<'_>,
    roll_results_elems: &mut Select<'_, '_>,
    rolled_elem: &ElementRef<'_>,
) -> Option<Roll> {
    let expr_outcome = rolled_elem
        .text()
        .collect::<Vec<&str>>()
        .join("")
        .parse::<f64>()
        .ok()?;
    let expr_string = formula_elem
        .text()
        .collect::<Vec<&str>>()
        .join("")
        .replace("rolling ", "");
    let outcomes: Vec<i64> = roll_results_elems
        .filter_map(|frag| {
            frag.text()
                .collect::<Vec<&str>>()
                .join("")
                .parse::<i64>()
                .ok()
        })
        .collect();

    get_roll_from_expression_and_outcomes(expr_string.as_str(), outcomes, expr_outcome)
}

fn try_get_roll_from_macro(roll_result_elem: &ElementRef) -> Option<Roll> {
    let expr_outcome = roll_result_elem
        .text()
        .collect::<Vec<&str>>()
        .join("")
        .parse::<f64>()
        .ok()?;

    let expr_raw = roll_result_elem.value().attr("title")?;
    let expr_fragment = Html::parse_fragment(expr_raw);
    let results_elems_selector = Selector::parse(".basicdiceroll").unwrap();
    let outcomes: Vec<i64> = expr_fragment
        .select(&results_elems_selector)
        .map(|frag| {
            frag.text()
                .collect::<Vec<&str>>()
                .join("")
                .parse::<i64>()
                .unwrap()
        })
        .collect();

    let equals_position = expr_raw
        .find(" = ")
        .expect("inlinerollresult span attribute 'title' does not include equals sign");
    let expr_string = expr_raw[..equals_position].replace("Rolling ", "");

    get_roll_from_expression_and_outcomes(expr_string.as_str(), outcomes, expr_outcome)
}

fn get_rolls_from_fragment(fragment: &Html) -> Vec<Roll> {
    let plain_formula_selector = Selector::parse(".formula").unwrap();
    let plain_results_selector = Selector::parse(".dicegrouping .didroll").unwrap();
    let plain_rolled_selector = Selector::parse(".rolled").unwrap();
    let macro_formula_selector = Selector::parse(".inlinerollresult").unwrap();

    let mut rolls: Vec<Roll> = vec![];

    if let Some(formula_elem) = fragment.select(&plain_formula_selector).next() {
        let mut roll_results_elems = fragment.select(&plain_results_selector);
        if let Some(rolled_elem) = fragment.select(&plain_rolled_selector).next() {
            if let Some(roll) =
                try_get_roll_from_plain(&formula_elem, &mut roll_results_elems, &rolled_elem)
            {
                rolls.push(roll);
            }
        }
    }

    let mut roll_result_elems = fragment.select(&macro_formula_selector);
    while let Some(roll_result_elem) = roll_result_elems.next() {
        if let Some(roll) = try_get_roll_from_macro(&roll_result_elem) {
            rolls.push(roll);
        }
    }

    rolls
}

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
        let private_message_selector = Selector::parse(".message.private").unwrap();
        let general_message_selector = Selector::parse(".message.general").unwrap();
        let full_message_selector = Selector::parse(".message").unwrap();
        let sender_selector = Selector::parse(".by").unwrap();
        let timestamp_selector = Selector::parse(".tstamp").unwrap();

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

        let is_private_message = fragment.select(&private_message_selector).next().is_some();
        if is_private_message {
            return None;
        }

        let full_message = fragment.select(&full_message_selector).next()?.value();
        let id = full_message.attr("data-messageid")?.to_string();
        let sender_name = self.last_parsed_sender_name.clone().unwrap();
        let datetime = self.last_parsed_datetime.clone().unwrap();
        let mut content_raw = String::from("");
        let mut is_message = false;
        let rolls = get_rolls_from_fragment(&fragment);

        let general_message = fragment.select(&general_message_selector).next();
        if general_message.is_some() && rolls.len() == 0 {
            for text in general_message.unwrap().text() {
                content_raw.drain(..);
                content_raw.push_str(text);
            }

            content_raw = trim_whitespace(content_raw.trim());

            if content_raw != sender_name && content_raw.len() > 0 {
                is_message = true;
            }
        }

        let post = Post {
            id,
            sender_name,
            datetime,
            content_raw,
            is_message,
            rolls,
        };

        Some(post)
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
                    if grapheme == "<" {
                        current_tag.push_str(grapheme);
                    } else if grapheme == ">" {
                        let opening_count = current_tag.chars().filter(|ch| *ch == '<').count();
                        let closing_count = current_tag.chars().filter(|ch| *ch == '>').count();
                        if opening_count - closing_count > 1 {
                            current_tag.push_str(grapheme);
                            continue;
                        }

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
                            if post.is_some() {
                                return post;
                            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_roll_from_plain() {
        let raw_roll_html = r#"
            <div class="message general" data-messageid="-Tes--1-tEsTIDFFFFFF">
                <div class="sheet-rolltemplate-simple">
                <div class="sheet-container">
                    <div class="sheet-result">
                    <div class="sheet-solo">
                        <span
                        ><span
                            class="inlinerollresult showtip tipsy-n-right"
                            title='Rolling 1d20+0[Mods] = (<span class="basicdiceroll">13</span>)+0'
                            >13</span
                        ></span
                        >
                    </div>
                    </div>
                    <div class="sheet-label">
                    <span>INSIGHT <span>(0)</span></span>
                    </div>
                </div>
                </div>
            </div>"#;
        let fragment = Html::parse_fragment(raw_roll_html);

        let rolls = get_rolls_from_fragment(&fragment);
        assert_eq!(rolls.len(), 1);
        assert_eq!(rolls[0].outcome, 13.);
        assert_eq!(rolls[0].single_rolls.len(), 1);
        assert_eq!(rolls[0].single_rolls[0].faces, 20);
        assert_eq!(rolls[0].single_rolls[0].outcome, 13);

        let raw_roll_html = r#"
            <div
                class="message rollresult player--tEsTiD12345"
                data-messageid="-Tes--1-tEsTIDFFFFFF"
                data-playerid="-tEsTiD12345"
            >
                <div class="formula" style="margin-bottom: 3px">rolling 4d6k3</div>
                <div class="clear"></div>
                <div class="formula formattedformula">
                <div class="dicegrouping" data-groupindex="0">
                    (
                    <div data-origindex="0" class="diceroll d6">
                    <div class="dicon">
                        <div class="didroll">3</div>
                        <div class="backing"></div>
                    </div>
                    +
                    </div>
                    <div data-origindex="1" class="diceroll d6 critfail">
                    <div class="dicon">
                        <div class="didroll">1</div>
                        <div class="backing"></div>
                    </div>
                    +
                    </div>
                    <div data-origindex="2" class="diceroll d6 dropped critfail">
                    <div class="dicon">
                        <div class="didroll">1</div>
                        <div class="backing"></div>
                    </div>
                    +
                    </div>
                    <div data-origindex="3" class="diceroll d6">
                    <div class="dicon">
                        <div class="didroll">3</div>
                        <div class="backing"></div>
                    </div>
                    </div>
                    )
                </div>
                <div class="clear"></div>
                </div>
                <div class="clear"></div>
                <strong>=</strong>
                <div class="rolled">7</div>
            </div>"#;
        let fragment = Html::parse_fragment(raw_roll_html);

        let rolls = get_rolls_from_fragment(&fragment);
        assert_eq!(rolls.len(), 1);
        assert_eq!(rolls[0].outcome, 7.);
        assert_eq!(rolls[0].single_rolls.len(), 4);
        assert_eq!(rolls[0].single_rolls[0].faces, 6);
        assert_eq!(rolls[0].single_rolls[0].outcome, 3);
        assert_eq!(rolls[0].single_rolls[1].outcome, 1);
        assert_eq!(rolls[0].single_rolls[2].outcome, 1);
        assert_eq!(rolls[0].single_rolls[3].outcome, 3);
    }

    #[test]
    fn get_roll_from_macro() {
        let raw_roll_html = r#"
            <div class="message general you" data-messageid="-Tes--1-tEsTIDFFFFFG">
                <div class="spacer"></div>
                <div class="avatar" aria-hidden="true"><img src="/users/avatar/test/none" /></div>
                <span class="tstamp" aria-hidden="true">4:27PM</span><span class="by">cool_guy 420:</span>
                <div class="sheet-rolltemplate-npcfullatk">
                <div class="sheet-container">
                    <div class="sheet-row sheet-header">
                    <span>Force Ballista</span>
                    </div>
                    <div class="sheet-row sheet-subheader">
                    <span class="sheet-italics">Cannon</span>
                    </div>
                    <div class="sheet-arrow-right"></div>
                    <div class="sheet-row">
                    <span class="sheet-italics sheet-translated" data-i18n="attack:-u">ATTACK:</span
                    ><span
                        ><span
                        class="inlinerollresult showtip tipsy-n-right"
                        title='Rolling 1d20+(15+0) = (<span class="basicdiceroll">10</span>)+(15+0)'
                        >25</span
                        ></span
                    >
                    </div>
                </div>
                <div class="sheet-container sheet-dmgcontainer sheet-damagetemplate">
                    <span class="sheet-italics sheet-translated" data-i18n="dmg:-u">DAMAGE:</span>
                    <span>
                    <span
                        class="inlinerollresult showtip tipsy-n-right"
                        title='Rolling 3d8+0 = (<span class="basicdiceroll">7</span>+<span class="basicdiceroll">6</span>+<span class="basicdiceroll">5</span>)+0'
                        >18</span
                    >

                    force damage
                    </span>
                    <div class="sheet-row">
                    <span class="sheet-desc">120 ft range, single target, ranged spell attack</span>
                    </div>
                </div>
                </div>
            </div>
            </div>"#;
        let fragment = Html::parse_fragment(raw_roll_html);

        let rolls = get_rolls_from_fragment(&fragment);
        assert_eq!(rolls.len(), 2);
        assert_eq!(rolls[0].outcome, 25.);
        assert_eq!(rolls[1].outcome, 18.);
        assert_eq!(rolls[0].single_rolls.len(), 1);
        assert_eq!(rolls[0].single_rolls[0].faces, 20);
        assert_eq!(rolls[0].single_rolls[0].outcome, 10);
        assert_eq!(rolls[1].single_rolls.len(), 3);
        assert_eq!(rolls[1].single_rolls[0].faces, 8);
        assert_eq!(rolls[1].single_rolls[0].outcome, 7);
        assert_eq!(rolls[1].single_rolls[1].outcome, 6);
        assert_eq!(rolls[1].single_rolls[2].outcome, 5);
    }
}
