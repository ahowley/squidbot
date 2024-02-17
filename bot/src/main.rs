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
    let message = controllers::dump_unmapped_senders().await;
    ctx.say(message).await?;
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

/// Grabs the names of all campaigns in the database for use in other commands.
#[poise::command(slash_command, prefix_command, aliases("camp"), category = "Utility")]
async fn campaigns(ctx: Context<'_>) -> Result<(), Error> {
    ctx.say(format!(
        "Here's a list of all the campaign's I know about: {}",
        controllers::campaigns().await.join(", ")
    ))
    .await?;
    Ok(())
}

/// Gets a random campaign quote, optionally from a specific campaign.
#[poise::command(slash_command, prefix_command, aliases("cq"), category = "Fun")]
async fn campaignquote(
    ctx: Context<'_>,
    #[description = "The name of the campaign you'd like to fetch from - use /campaigns to see the options!"]
    #[rest]
    campaign: Option<String>,
) -> Result<(), Error> {
    ctx.say(controllers::campaign_quote(campaign).await).await?;
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
    // This is our custom error handler
    // They are many errors that can occur, so we only handle the ones we want to customize
    // and forward the rest to the default handler
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
        serenity::FullEvent::Message { new_message } => {
            if new_message
                .mentions
                .iter()
                .map(|user| user.id)
                .collect::<Vec<serenity::UserId>>()
                .contains(&ctx.cache.current_user().id)
            {
                new_message
                    .reply(ctx, controllers::campaign_quote(None).await)
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
                update_chatlogs(),
                dump_unmapped_senders(),
                message(),
                campaigns(),
                campaignquote(),
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
