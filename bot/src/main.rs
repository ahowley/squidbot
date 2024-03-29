use futures::{executor::block_on, future, Stream, StreamExt};
use poise::{samples::HelpConfiguration, serenity_prelude as serenity};

mod controllers;

struct Data {}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

/// Returns true if username matches the configured bot owner
async fn is_owner_check(ctx: Context<'_>) -> Result<bool, Error> {
    let bot_owner = &std::env::var("BOT_OWNER").expect("missing BOT_OWNER");
    let username = &ctx.author().name;

    Ok(username == bot_owner)
}

#[poise::command(
    prefix_command,
    hide_in_help,
    check = "is_owner_check",
    category = "Utility"
)]
async fn byebye_bot(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("See ya.").await?;
    ctx.framework().shard_manager.shutdown_all().await;
    Ok(())
}

#[poise::command(
    prefix_command,
    hide_in_help,
    check = "is_owner_check",
    category = "Utility"
)]
async fn update_chatlogs(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("I'll get started now!").await?;
    controllers::update_chatlogs().await;
    ctx.say("All done!").await?;
    Ok(())
}

#[poise::command(
    prefix_command,
    hide_in_help,
    check = "is_owner_check",
    category = "Utility"
)]
async fn dump_unmapped_senders(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Collecting senders...").await?;
    let messages = controllers::dump_unmapped_senders().await;
    for message in messages {
        ctx.say(message).await?;
    }
    Ok(())
}

#[poise::command(
    prefix_command,
    hide_in_help,
    check = "is_owner_check",
    category = "Utility"
)]
pub async fn register(ctx: Context<'_>) -> Result<(), Error> {
    poise::builtins::register_application_commands_buttons(ctx).await?;
    Ok(())
}

/// I'll send you a random message!
#[poise::command(slash_command, prefix_command, category = "Fun")]
async fn message(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(controllers::message().await).await?;
    Ok(())
}

async fn autocomplete_campaign<'a>(
    _ctx: Context<'_>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    let campaigns = controllers::campaigns().await;

    futures::stream::iter(campaigns)
        .filter(move |name| futures::future::ready(name.starts_with(partial)))
        .map(|name| name.to_string())
}

async fn autocomplete_sender<'a>(
    _ctx: Context<'_>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    let senders = controllers::senders().await;

    futures::stream::iter(senders)
        .filter(move |name| futures::future::ready(name.starts_with(partial)))
        .map(|name| name.to_string())
}

async fn autocomplete_player<'a>(
    _ctx: Context<'_>,
    partial: &'a str,
) -> impl Stream<Item = String> + 'a {
    let players = controllers::players().await;

    futures::stream::iter(players)
        .filter(move |name| futures::future::ready(name.starts_with(partial)))
        .map(|name| name.to_string())
}

fn campaignquote_help() -> String {
    let (mut campaigns, mut players) = block_on(async {
        let results = future::join(controllers::campaigns(), controllers::players()).await;
        results
    });

    campaigns.sort_unstable_by(|a, b| a.cmp(&b));
    players.sort_unstable_by(|a, b| a.cmp(&b));
    format!(
        "Here's a list of all the campaigns I know about:\n```\n{}```
        
        Here's a list of all the players I know about:\n```\n{}```",
        campaigns.join(", "),
        players.join(", ")
    )
}

/// Gets a random campaign quote, with some optional filters.
#[poise::command(
    slash_command,
    aliases("cq"),
    help_text_fn = "campaignquote_help",
    category = "Fun"
)]
async fn campaignquote(
    ctx: Context<'_>,
    #[description = "The name of the campaign you'd like to fetch from!"]
    #[autocomplete = "autocomplete_campaign"]
    campaign: Option<String>,
    #[description = "The name of the sender who sent the message!"]
    #[autocomplete = "autocomplete_sender"]
    sender: Option<String>,
    #[description = "The name of the campaign you'd like to fetch from!"]
    #[autocomplete = "autocomplete_player"]
    player: Option<String>,
) -> Result<(), Error> {
    ctx.say(controllers::campaign_quote(campaign, sender, player).await)
        .await?;
    Ok(())
}

/// Find out who sent a message! You can also send ".whosent" or ".ws" when replying to a message.
#[poise::command(slash_command, aliases("ws"), category = "Fun")]
async fn whosent(
    ctx: Context<'_>,
    #[description = "The message to search for"] message: String,
) -> Result<(), Error> {
    let replies = controllers::who_sent(message).await;
    for reply in replies {
        ctx.say(reply).await?;
    }
    Ok(())
}

/// whosent command in context menu
#[poise::command(context_menu_command = "Who Sent")]
pub async fn whosent_context(ctx: Context<'_>, msg: serenity::Message) -> Result<(), Error> {
    let replies = controllers::who_sent(msg.content).await;
    for reply in replies {
        ctx.say(reply).await?;
    }
    Ok(())
}

/// Search for any text in a message! You can also send ".search" when replying to a message.
#[poise::command(slash_command, category = "Fun")]
async fn search(
    ctx: Context<'_>,
    #[description = "The message to search for"] message: String,
    #[description = "The maximum number of results (up to 10)"]
    #[min = 1]
    #[max = 10]
    limit: Option<i32>,
) -> Result<(), Error> {
    let reply = controllers::search(message, limit).await;
    ctx.say(reply).await?;
    Ok(())
}

/// search command in context menu
#[poise::command(context_menu_command = "Search Message")]
pub async fn search_context(ctx: Context<'_>, msg: serenity::Message) -> Result<(), Error> {
    let reply = controllers::search(msg.content, None).await;
    ctx.say(reply).await?;
    Ok(())
}

/// See the context around a message! A message's ID will show up when you use "/whosent".
#[poise::command(slash_command, aliases("a"), category = "Fun")]
async fn around(
    ctx: Context<'_>,
    #[description = "The message ID to see around"] message_id: String,
    #[description = "The number of messages to see before and after this message (to a max of 5)"]
    #[min = 1]
    #[max = 5]
    num_around: Option<i32>,
) -> Result<(), Error> {
    let reply = controllers::around(message_id, num_around.unwrap_or(1)).await;
    ctx.say(reply).await?;
    Ok(())
}

/// Roll some dice or do some math (or both!)
#[poise::command(
    slash_command,
    prefix_command,
    aliases("r", "rm", "m", "math"),
    category = "Nerd"
)]
async fn roll(
    ctx: Context<'_>,
    #[description = "The message to search for"]
    #[rest]
    expr: String,
) -> Result<(), Error> {
    ctx.say(controllers::roll(expr.as_str()).await).await?;
    Ok(())
}

/// Get an estimated odds of rolling a certain result (out of 1 million rolls).
#[poise::command(slash_command, prefix_command, aliases("o"), category = "Nerd")]
async fn odds(
    ctx: Context<'_>,
    #[description = "The outcome of the roll"] result: String,
    #[description = "The roll being made"]
    #[rest]
    expr: String,
) -> Result<(), Error> {
    let res_float_option = result.parse::<f64>().ok();
    if res_float_option.is_none() {
        ctx.say(
            "Sorry, I couldn't get the estimated odds of rolling `{result}` or higher on: `{expr}`",
        )
        .await?;
    } else {
        let res_float = res_float_option.unwrap();
        ctx.say(controllers::odds(expr.as_str(), res_float, 1_000_000).await)
            .await?;
    }
    Ok(())
}

/// Get an estimated odds of rolling a certain result (out of 100 million rolls).
#[poise::command(prefix_command, aliases("op"), category = "Nerd")]
async fn odds_precise(
    ctx: Context<'_>,
    #[description = "The outcome of the roll"] result: String,
    #[description = "The roll being made"]
    #[rest]
    expr: String,
) -> Result<(), Error> {
    let res_float_option = result.parse::<f64>().ok();
    if res_float_option.is_none() {
        ctx.say(
            "Sorry, I couldn't get the estimated odds of rolling `{result}` or higher on: `{expr}`",
        )
        .await?;
    } else {
        let res_float = res_float_option.unwrap();
        ctx.say(controllers::odds(expr.as_str(), res_float, 100_000_000).await)
            .await?;
    }
    Ok(())
}

fn luck_help() -> String {
    let mut players = block_on(async { controllers::players().await });

    players.sort_unstable_by(|a, b| a.cmp(&b));
    format!(
        "Here's a list of all the players I know about:\n```\n{}```",
        players.join(", ")
    )
}

/// Find out whether I'm luckier than a given player!
#[poise::command(
    slash_command,
    aliases("l"),
    help_text_fn = "luck_help",
    category = "Fun"
)]
async fn luck(
    ctx: Context<'_>,
    #[description = "The player to compete with"]
    #[autocomplete = "autocomplete_player"]
    player: String,
) -> Result<(), Error> {
    ctx.say(controllers::simulate(player.as_str(), 1).await)
        .await?;
    Ok(())
}

/// Simulate every dice roll a player has made 10,000 times to TRULY find out how lucky they are.
#[poise::command(
    prefix_command,
    aliases("s"),
    help_text_fn = "luck_help",
    category = "Fun"
)]
async fn simulate(
    ctx: Context<'_>,
    #[description = "The name of the player to compete with"] player: String,
) -> Result<(), Error> {
    ctx.say("I'm working on it - this may take a while 👀")
        .await?;
    ctx.say(controllers::simulate(player.as_str(), 10_000).await)
        .await?;
    Ok(())
}

/// Search for the worst roll in all of history.
#[poise::command(prefix_command, aliases("cn1"), category = "Fun")]
async fn cosmicnat1(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("If you insist... gimme a sec.").await?;
    ctx.say(controllers::worst_roll(false).await).await?;
    Ok(())
}

/// Search for greatest roll of all time.
#[poise::command(prefix_command, aliases("cn20"), category = "Fun")]
async fn cosmicnat20(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Okay, gimme a sec!").await?;
    ctx.say(controllers::best_roll(false).await).await?;
    Ok(())
}

/// The (extremely) slow, but more precise, version of .cosmicnat1
#[poise::command(prefix_command, aliases("cn1p"), category = "Fun")]
async fn cosmicnat1_precise(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("If you insist... this is gonna take a minute. I would appreciate some space.")
        .await?;
    ctx.say(controllers::worst_roll(true).await).await?;
    Ok(())
}

/// The (extremely) slow, but more precise, version of .cosmicnat20
#[poise::command(prefix_command, aliases("cn20p"), category = "Fun")]
async fn cosmicnat20_precise(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say("Okay, this is where the fun begins! You'll need to give me... a minute or two.")
        .await?;
    ctx.say(controllers::best_roll(true).await).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command, track_edits, category = "Utility")]
async fn help(
    ctx: Context<'_>,
    #[description = "Command to get help for"]
    #[rest]
    mut command: Option<String>,
) -> Result<(), Error> {
    if ctx.invoked_command_name() != "help" {
        command = match command {
            Some(c) => Some(format!("{} {}", ctx.invoked_command_name(), c)),
            None => Some(ctx.invoked_command_name().to_string()),
        };
    }
    let extra_text_at_bottom = "\
Type `.help command` for more info on a command.
You can edit your `.help` message and I'll edit my response.";

    let config = HelpConfiguration {
        show_subcommands: true,
        show_context_menu_commands: true,
        ephemeral: true,
        extra_text_at_bottom,

        ..Default::default()
    };
    poise::builtins::help(ctx, command.as_deref(), config).await?;
    Ok(())
}

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Setup { error, .. } => panic!("Failed to start bot: {:?}", error),
        poise::FrameworkError::Command { error, ctx, .. } => {
            println!("Error in command `{}`: {:?}", ctx.command().name, error,);
        }
        error => {
            if let Err(e) = poise::builtins::on_error(error).await {
                println!("Error while handling error: {}", e)
            }
        }
    }
}

async fn event_handler(
    ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    _data: &Data,
) -> Result<(), Error> {
    match event {
        serenity::FullEvent::Ready { data_about_bot, .. } => {
            println!("Logged in as {}", data_about_bot.user.name);
        }
        serenity::FullEvent::ReactionAdd { add_reaction } => {
            if add_reaction.user(ctx).await?.id == ctx.cache.current_user().id {
                return Ok(());
            }
            let message = add_reaction.message(ctx).await?;
            let emoji = &add_reaction.emoji;
            match emoji {
                serenity::ReactionType::Unicode(content) => {
                    if content == "🦑" {
                        println!("Responding to squid reaction");
                        message.react(ctx, emoji.clone()).await?;
                    }
                }
                _ => (),
            }
        }
        serenity::FullEvent::Message { new_message } => {
            let bot_mentioned = new_message
                .mentions
                .iter()
                .map(|user| user.id)
                .collect::<Vec<serenity::UserId>>()
                .contains(&ctx.cache.current_user().id);
            if let Some(replied_to) = &new_message.referenced_message {
                if new_message.content.starts_with(".whosent")
                    || new_message.content.starts_with(".ws")
                {
                    println!("Executing response to whosent reply");
                    let search_message = &replied_to.content;
                    let replies = controllers::who_sent(search_message.clone()).await;
                    for reply in replies {
                        new_message.reply(ctx, reply).await?;
                    }
                } else if new_message.content.starts_with(".search") {
                    println!("Executing response to search reply");
                    let search_message = &replied_to.content;
                    let reply = controllers::search(search_message.clone(), None).await;
                    new_message.reply(ctx, reply).await?;
                } else if &replied_to.author.id == &ctx.cache.current_user().id || bot_mentioned {
                    println!("Executing response to bot reply");
                    new_message
                        .reply(ctx, controllers::campaign_quote(None, None, None).await)
                        .await?;
                }
            } else if bot_mentioned {
                println!("Executing response to bot mention");
                new_message
                    .reply(ctx, controllers::campaign_quote(None, None, None).await)
                    .await?;
            }
        }
        _ => {}
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");
    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                byebye_bot(),
                update_chatlogs(),
                dump_unmapped_senders(),
                message(),
                campaignquote(),
                whosent(),
                whosent_context(),
                search(),
                search_context(),
                around(),
                roll(),
                odds(),
                odds_precise(),
                luck(),
                simulate(),
                cosmicnat1(),
                cosmicnat20(),
                cosmicnat1_precise(),
                cosmicnat20_precise(),
                help(),
                register(),
            ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some(".".into()),
                ..Default::default()
            },
            on_error: |error| Box::pin(on_error(error)),
            pre_command: |ctx| {
                Box::pin(async move {
                    println!("Executing command {}...", ctx.command().qualified_name);
                })
            },
            post_command: |ctx| {
                Box::pin(async move {
                    println!("Executed command {}!", ctx.command().qualified_name);
                })
            },
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data {})
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;

    client.unwrap().start().await.unwrap();
}
