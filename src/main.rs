use std::collections::HashMap;
use std::sync::Arc;
use std::sync::OnceLock;

use poise::serenity_prelude::EventHandler;
use poise::serenity_prelude::{self as serenity, Member, GuildMemberUpdateEvent};

use crate::modules::antispam;
use crate::modules::honeypot;
mod modules;
mod utils;

#[derive(Debug, serde::Deserialize)]
struct Settings {
    // Where logs should be sent for each server
    log_channels: HashMap<serenity::GuildId, serenity::ChannelId>,
    // Roles that only bots will assign themselves
    honeypot_roles: HashMap<serenity::GuildId, serenity::RoleId>,
    // Roles that users can assign themselves
    // if they have other roles, they do not get for assigning honeypot roles
    // but a notification will be sent to that server's log channel
    user_assignable_roles: HashMap<serenity::GuildId, Vec<serenity::RoleId>>
}

struct Data {}
static CONTEXT: OnceLock<Arc<poise::serenity_prelude::Context>> = OnceLock::new();
static SETTINGS: OnceLock<Arc<Settings>> =
    OnceLock::new();

type Error = Box<dyn std::error::Error + Send + Sync>;

type Context<'a> = poise::Context<'a, Data, Error>;

#[poise::command(slash_command)]
async fn test_command(ctx: Context<'_>) -> Result<(), Error> {
    ctx.send(poise::CreateReply {
        content: Some("Test command".to_string()),
        ephemeral: Some(true),
        ..Default::default()
    })
    .await?;
    Ok(())
}


struct Handler;

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn message(&self, ctx: serenity::Context, msg: serenity::Message) {
        antispam::on_message(&ctx, &msg).await;
    }
    async fn guild_member_update(&self, ctx: serenity::Context, old_if_available: Option<Member>, new: Option<Member>, event: GuildMemberUpdateEvent) {
        honeypot::on_member_update(&ctx, old_if_available.as_ref(), new.as_ref(), &event).await;
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("Starting Autokatze");
    antispam::start();
    dotenvy::dotenv().ok();
    let token = std::env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set");
    let log_channels_location =
        std::env::var("LOG_CHANNELS").expect("LOG_CHANNELS must be set to a valid path");
    let intents = serenity::GatewayIntents::all();

    let settings = std::fs::read_to_string(log_channels_location).expect("File must exist");
    let settings: Settings =
        serde_json::from_str(&settings).expect("Invalid log channels JSON");
    SETTINGS.set(Arc::new(settings)).unwrap();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![test_command()],
            ..Default::default()
        })
        .setup(|ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                let _ = CONTEXT.set(Arc::new(ctx.clone()));
                Ok(Data {})
            })
        })
        .build();
    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .event_handler(Handler)
        .await;
    client.unwrap().start().await.unwrap();
}
