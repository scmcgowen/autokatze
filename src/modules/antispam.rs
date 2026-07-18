use crate::SETTINGS;
use crate::{CONTEXT, utils};
use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{
    ChannelId, CreateEmbed, CreateMessage, GuildId, Mentionable,
};
use poise::serenity_prelude::{Message, UserId};
use sha2::Digest;
use sha2::Sha256;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::Mutex;
static ANTISPAM_THRESHOLD_CHANNELS: usize = 3;
static ANTISPAM_THRESHOLD_SECONDS: u32 = 60;




#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey(String);

impl CacheKey {
    fn new(guild: u64, member: u64, hash: &str) -> Self {
        Self(format!("antispam:{}/{}/{}", guild, member, hash))
    }
}

#[derive(Debug, Clone,)]
struct AntispamCacheEntry {
    guild: GuildId,
    channels: Vec<u64>,
    messages: Vec<Message>,
    first_updated: DateTime<Utc>,
    last_updated: DateTime<Utc>,
    needs_kick: bool,
}

static ANTISPAM_CACHE: LazyLock<Mutex<HashMap<CacheKey, AntispamCacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static VIOLATORS: LazyLock<Mutex<HashMap<UserId, AntispamCacheEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub fn start() {
    tracing::info!("Starting antispam module");
    let mut clean_cache_intervel = tokio::time::interval(Duration::seconds(ANTISPAM_THRESHOLD_SECONDS as i64 * 2).to_std().unwrap());
    // Check for violators needing to be kicked every 1.5 seconds
    let mut check_violators_interval = tokio::time::interval(Duration::milliseconds(1500).to_std().unwrap());

    tokio::spawn(async move {
        loop {
            clean_cache_intervel.tick().await;
            clean_cache();
        }
    });
    tokio::spawn(async move {
        loop {
            check_violators_interval.tick().await;
            check_violators().await;
        }
    });
}

fn insert_message(message: Message) {
    let digest = Sha256::digest(message.content.as_bytes());
    let hash = hex::encode(digest.as_slice());
    let key = CacheKey::new(
        message.guild_id.map_or(0, |g| g.get()),
        message.author.id.get(),
        &hash,
    );
    {
        let mut cache = ANTISPAM_CACHE.lock().unwrap();
        let entry = cache.entry(key).or_insert_with(|| {
            AntispamCacheEntry {
                guild: message.guild_id.unwrap_or_default(),
                channels: vec![], // empty vectors, will be populated soon
                messages: vec![], // intentionally empty to prevent duplicates
                first_updated: Utc::now(),
                last_updated: Utc::now(),
                needs_kick: false,
            }
        });
        entry.last_updated = Utc::now();
        entry.channels.push(message.channel_id.get());
        let author_id = message.author.id.clone();
        entry.messages.push(message);
        if !entry.needs_kick {
            if entry.messages.len() >= ANTISPAM_THRESHOLD_CHANNELS {
                entry.needs_kick = true;
                let mut violators = VIOLATORS.lock().unwrap();
                violators.insert(author_id, entry.clone());
            }
        }
    }
}

fn clean_cache() {
    let now = Utc::now();
    let mut cache = ANTISPAM_CACHE.lock().unwrap();
    let mut queue = Vec::new();
    for (key, entry) in cache.iter_mut() {
        if now - entry.last_updated >= Duration::seconds(ANTISPAM_THRESHOLD_SECONDS as i64) {
            queue.push(key.clone());
        }
    }
    for key in queue {
        cache.remove(&key);
    }
}

async fn check_violators() {
    let mut violators = VIOLATORS.lock().unwrap();
    let mut queue = Vec::new();
    for (key, entry) in violators.iter_mut() {
        if entry.needs_kick {
            tokio::spawn(annihilate_violator(entry.clone(), key.clone()));
            queue.push(key.clone())
        }
    }
    for key in queue {
        violators.remove(&key);
    }

}

async fn annihilate_violator(entry:AntispamCacheEntry, user_id: UserId) {
    //TODO: log violators
    let log_channels = SETTINGS.get().unwrap();
    if let Some(channel_id) = log_channels.log_channels.get(&entry.guild) {
        let http = CONTEXT.get().unwrap().http.clone();
        let member = entry.guild.member(http.clone(), user_id).await;
        let member = match member {
            Ok(member) => member,
            Err(_) => return,
        };
        let channels = entry.guild.channels(http.clone()).await.unwrap();
        let channel = channels.get(channel_id);

        match channel {
            Some(channel) => {
                channel.send_message(http.clone(), CreateMessage::default()
                    .content(format!("User {} was kicked for violating the antispam rules", user_id))
                    .embed(CreateEmbed::default()
                        .field("Spammer kicked.",format!("{} / {}\n(Sent the same message to {} channels within {} seconds)", member.display_name(),member.user.name, entry.channels.len(), ANTISPAM_THRESHOLD_SECONDS) , false)
                        .field("Channels", format!("{}", entry.channels.iter().map(|c| ChannelId::new(c.to_owned()).mention().to_string()).collect::<Vec<_>>().join(", ")), false)
                        .field("Times", format!("{} - {}", entry.first_updated, entry.last_updated), false)
                        .field("Content:", entry.messages[0].content.clone(), false)
                    )
                ).await.unwrap();
            }
            None => {}
        }
    }

    let http = CONTEXT.get().unwrap().http.clone();
    let _ = entry.guild.kick(http.clone(), user_id).await;
    for message in &entry.messages {
        // ignore errors, messages may have already been deleted
        let _ = message.delete(http.clone()).await;
    }
}

pub async fn on_message(ctx: &serenity::Context, msg: &serenity::Message) {
    if msg.author.bot {
        // ignore bot messages
        return;
    }
    let member = msg.member(ctx).await;
    if member.is_err() {
        return;
    }
    let member = member.unwrap();
    // ignore staff members
    if utils::is_staff(&member, ctx).await {
        return;
    }

    insert_message(msg.clone());
}
