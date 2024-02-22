use rand::seq::SliceRandom;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use sqlx::types::chrono::FixedOffset;
use std::sync::Arc;

pub async fn message() -> String {
    parse::get_random_message("./random_message_templates.json".to_string()).await
}

pub async fn update_chatlogs() {
    let pool = Arc::new(data::create_connection_pool("./.env").await);
    let config = parse::parse_config("./config.json".to_string()).await;

    let mut transaction = data::begin_transaction(&pool).await;
    data::update_players(&mut transaction, &config).await;
    data::update_campaigns(&mut transaction, &config).await;
    transaction
        .commit()
        .await
        .expect("failed to commit transaction");

    for (campaign_name, campaign_config) in &config.campaigns {
        // TODO: refactor ChatLogs to bake offset into db to avoid the need to check this so much
        let offset = if campaign_config.log.starts_with("fnd_") {
            None
        } else {
            Some(campaign_config.timezone_offset)
        };

        println!("Updating campaign: {campaign_name}");
        data::update_posts_from_log(
            &pool,
            campaign_name,
            "./chatlogs",
            campaign_config.log.as_str(),
            offset,
        )
        .await;
    }
}

pub async fn dump_unmapped_senders() -> String {
    let config = parse::parse_config("./config.json".to_string()).await;
    let senders_map = data::dump_unmapped_senders(&config).await;
    let mut message = "```".to_string();

    for (campaign, senders) in senders_map {
        message.push_str(&format!("--------{campaign}--------\n"));
        for sender in senders {
            message.push_str(&format!("\"{sender}\",\n"));
        }
    }
    message.push_str("```");

    message
}

pub async fn campaigns() -> Vec<String> {
    let pool = data::create_connection_pool("./.env").await;
    data::fetch_campaign_names(&pool).await
}

pub async fn senders() -> Vec<String> {
    let pool = data::create_connection_pool("./.env").await;
    data::fetch_sender_names(&pool).await
}

pub async fn players() -> Vec<String> {
    let pool = data::create_connection_pool("./.env").await;
    data::fetch_player_names(&pool).await
}

pub async fn campaign_quote(
    campaign: Option<String>,
    sender: Option<String>,
    player: Option<String>,
) -> String {
    let pool = data::create_connection_pool("./.env").await;
    let config = parse::parse_config("./config.json".to_string()).await;

    let campaign_name = campaign.unwrap_or("".to_string());
    let sender_name = sender.unwrap_or("".to_string());
    let player_name = player.unwrap_or("".to_string());

    let result =
        data::fetch_random_chat_message(&pool, &config, &campaign_name, &sender_name, &player_name)
            .await;

    if result.len() > 0 {
        result
    } else {
        "Sorry, I couldn't find any quotes with these filters!".to_string()
    }
}

pub async fn who_sent(message: String) -> String {
    let pool = data::create_connection_pool("./.env").await;

    if let Some(results) = data::trace_message(&pool, &message).await {
        let mut response = format!("Here's everyone who sent '`{}`':\n\n", message);
        response.push_str(
            &results
                .into_iter()
                .map(|trace| format!("```{trace}```"))
                .collect::<Vec<String>>()
                .join("\n"),
        );

        return response;
    }

    return format!("Sorry - I couldn't find '`{}`' in the database!", message);
}

pub async fn around(message_id: String, num_around: i32) -> String {
    let pool = data::create_connection_pool("./.env").await;
    let config = parse::parse_config("./config.json".to_string()).await;

    if let Some(results) =
        data::trace_around_message(&pool, &config, message_id.as_str(), num_around).await
    {
        let mut response = format!("Here's the context:\n\n");
        response.push_str(
            &results
                .into_iter()
                .map(|trace| format!("```{trace}```"))
                .collect::<Vec<String>>()
                .join("\n"),
        );

        return response;
    }

    return format!(
        "Sorry - I couldn't find '`{}`' in the database!",
        message_id
    );
}

pub async fn roll(expr: &str) -> String {
    let result = parse::dicemath(expr);

    match result {
        Some(value) => format!("Result for `{expr}`:\n`{value}`"),
        None => format!("Sorry, I couldn't figure out how to parse `{expr}`!"),
    }
}

pub async fn odds(expr: &str, val: f64, num_rolls: u64) -> String {
    let results: Vec<bool> = (0..num_rolls)
        .into_par_iter()
        .filter_map(|_| {
            let result = parse::dicemath(expr)?;
            if result >= val {
                Some(true)
            } else {
                None
            }
        })
        .collect();

    format!(
        "Out of `{}` rolls of `{expr}`, I rolled a `{val}` or higher a total of `{}` times.\n\nMy estimated odds of rolling a `{val}` or higher is about `{}`%!",
        parse::num_with_thousands_commas(num_rolls as u64),
        parse::num_with_thousands_commas(results.len() as u64),
        results.len() as f64 / (num_rolls as f64) * 100.
    )
}

pub async fn simulate(player_name: &str, num_repetitions: i32) -> String {
    let pool = data::create_connection_pool("./.env").await;
    let all_pronouns = data::fetch_player_pronouns(&pool, player_name).await;
    let pronouns = all_pronouns.choose(&mut rand::thread_rng()).unwrap();

    let rolls = data::fetch_all_single_rolls(&pool, player_name).await;

    if rolls.len() == 0 {
        return "Sorry, I couldn't find any rolls for {player_name}!".to_string();
    }

    let mut num_rolled: u64 = 0;
    let mut num_beat: u64 = 0;
    let mut num_tied: u64 = 0;

    for parse::RollSingle { faces, outcome } in rolls {
        let results = (0..num_repetitions)
            .into_par_iter()
            .filter_map(|_| {
                let result = parse::dicemath(format!("1d{faces}").as_str())?;
                if result > outcome as f64 {
                    Some(true)
                } else if result == outcome as f64 {
                    Some(false)
                } else {
                    None
                }
            })
            .collect::<Vec<bool>>();

        let ties: Vec<&bool> = results.iter().filter(|res| !**res).collect();

        num_rolled += num_repetitions as u64;
        num_tied += ties.len() as u64;
        num_beat += results.len() as u64 - ties.len() as u64;
    }

    format!(
        "\
I rolled every dice {player_name} has ever rolled {num_repetitions} time(s) each.
Out of {} rolls total, I beat {} {} times, or about {}% of the time.
We tied {} times, or about {}% of the time.
I'd estimate {player_name}'s luck to be about {}% of perfect.",
        parse::num_with_thousands_commas(num_rolled),
        pronouns[1],
        parse::num_with_thousands_commas(num_beat),
        (num_beat as f64 / num_rolled as f64 * 10000.).round() / 100.,
        parse::num_with_thousands_commas(num_tied),
        (num_tied as f64 / num_rolled as f64 * 10000.).round() / 100.,
        100. - (((num_beat as f64 / num_rolled as f64)
            + (num_tied as f64 / num_rolled as f64 / 2.))
            * 10000.)
            .round()
            / 100.
    )
}

// TODO: Refactor worst_roll & best_roll shared behavior
pub async fn worst_roll(precise: bool) -> String {
    let num_trials: usize = if precise { 100_000 } else { 1000 };

    let pool = data::create_connection_pool("./.env").await;
    let all_rolls = data::fetch_all_parseable_rolls(&pool).await;
    let mut odds: Vec<_> = all_rolls
        .into_par_iter()
        .map(
            |(player_name, campaign_name, formula, outcome, timestamp_sent, timezone_offset)| {
                let cmp_outcome = outcome + 1.;

                let results: Vec<bool> = (0..num_trials)
                    .into_iter()
                    .filter_map(|_| {
                        let result = parse::dicemath(formula.as_str())?;
                        if result >= cmp_outcome {
                            Some(true)
                        } else {
                            None
                        }
                    })
                    .collect();

                (
                    player_name,
                    campaign_name,
                    formula,
                    outcome,
                    timestamp_sent,
                    timezone_offset,
                    100. - results.len() as f64 / num_trials as f64 * 100.,
                )
            },
        )
        .collect();
    odds.sort_unstable_by(|a, b| {
        let odds_1 = a.6;
        let odds_2 = b.6;

        odds_1.total_cmp(&odds_2)
    });

    let (
        player_name,
        campaign_name,
        formula,
        outcome,
        timestamp_sent,
        timezone_offset,
        odds_this_bad,
    ) = &odds[0];

    let fixed_offset = FixedOffset::east_opt(timezone_offset * 3600).unwrap();
    let offset_timezone = *timestamp_sent + fixed_offset;
    let date = offset_timezone.date_naive().format("%m/%d/%Y");
    let time = offset_timezone.time().format("%-I:%M %p");

    format!("\
The worst single roll anyone has ever rolled was from {player_name} in \"{campaign_name}\" on {date} at {time}.
They rolled `{formula}` and got a `{outcome}`, which I estimated to have a `{:?}`% chance of being this bad.", odds_this_bad)
}

pub async fn best_roll(precise: bool) -> String {
    let num_trials: usize = if precise { 100_000 } else { 1000 };

    let pool = data::create_connection_pool("./.env").await;
    let all_rolls = data::fetch_all_parseable_rolls(&pool).await;
    let mut odds: Vec<_> = all_rolls
        .into_par_iter()
        .map(
            |(player_name, campaign_name, formula, outcome, timestamp_sent, timezone_offset)| {
                let results: Vec<bool> = (0..num_trials)
                    .into_iter()
                    .filter_map(|_| {
                        let result = parse::dicemath(formula.as_str())?;
                        if result >= outcome {
                            Some(true)
                        } else {
                            None
                        }
                    })
                    .collect();

                (
                    player_name,
                    campaign_name,
                    formula,
                    outcome,
                    timestamp_sent,
                    timezone_offset,
                    results.len() as f64 / num_trials as f64 * 100.,
                )
            },
        )
        .collect();
    odds.sort_unstable_by(|a, b| {
        let odds_1 = a.6;
        let odds_2 = b.6;

        odds_1.total_cmp(&odds_2)
    });

    let (
        player_name,
        campaign_name,
        formula,
        outcome,
        timestamp_sent,
        timezone_offset,
        odds_this_good,
    ) = odds[0].clone();

    let fixed_offset = FixedOffset::east_opt(timezone_offset * 3600).unwrap();
    let offset_timezone = timestamp_sent + fixed_offset;
    let date = offset_timezone.date_naive().format("%m/%d/%Y");
    let time = offset_timezone.time().format("%-I:%M %p");

    format!("\
The best single roll ever recorded was from {player_name} in \"{campaign_name}\" on {date} at {time}.
They rolled `{formula}` and got a `{outcome}`, which I estimated to have a `{:?}`% chance of being this good.", odds_this_good)
}
