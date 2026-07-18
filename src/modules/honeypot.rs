use poise::serenity_prelude::{self as serenity, GuildMemberUpdateEvent, Member};
use poise::serenity_prelude::{
    ChannelId, CreateEmbed, CreateMessage, GuildId, Mentionable,
};
use crate::CONTEXT;

use crate::{SETTINGS, utils};

pub async fn on_member_update(ctx: &serenity::Context, old_if_available: Option<&Member>, new: Option<&Member>, event: &GuildMemberUpdateEvent) {
    // if the new variant of the member doesn't exist somehow, ignore, but throw a log
    let member = match new {
        Some(m) => m,
        None => {
            tracing::warn!("Member update event received with no new member variant");
            return;
        }
    };
    tracing::info!("Member update event received");
    // if the member is staff, ignore
    if utils::is_staff(member, ctx).await {
        tracing::info!("User {} is staff, ignoring", member.user.id);
        return;
    }


    // get list of roles
    let roles = member.roles(ctx);
    let honeypot_role = SETTINGS.get().unwrap().honeypot_roles.get(&member.guild_id);
    let honeypot_role = match honeypot_role {
        Some(r) => r,
        None => {
            tracing::warn!("No honeypot role set for this guild");
            return;
        }
    }.clone();
    let self_assignable_roles = SETTINGS.get().unwrap().user_assignable_roles.get(&member.guild_id);
    let self_assignable_roles = match self_assignable_roles {
        Some(r) => r,
        None => &Vec::new(),
    };

    if let Some(old) = old_if_available {
        // get list of old roles
        let old_roles = old.roles(ctx);
        // compare roles and take action if necessary
        match old_roles {
            Some(roles) => {
                // they had roles before, compare with honeypot role
                if roles.iter().any(|r| r.id == honeypot_role) {
                    // they had the honeypot role before, ignore since we didn't kick them when it was first assigned, so its probably not a spammer, or they might've had other roles before, which makes us stop anyway
                    return;
                }
            }
            None => {} // they had no roles before?
        }
    }
    // they didn't have the honeypot role before, so we can proceed with the honeypot check
    match roles {
        Some(roles) => {
            if roles.iter().any(|r| r.id == honeypot_role) {
                tracing::info!("User {} has the honeypot role", member.user.id);
                // They have honeypot role, so we should check if they have other non-self-assignable roles
                // if they do, we should notify staff, but not kick them
                if roles.iter().any(|r| !self_assignable_roles.contains(&r.id) && r.id != honeypot_role) {
                    // they have a non-self-assignable role, notify staff but don't kick them
                    tracing::info!("User {} has a non-self-assignable role", member.user.id);
                    //Get the channel to log to
                    let log_channels = SETTINGS.get().unwrap();
                    if let Some(channel_id) = log_channels.log_channels.get(&member.guild_id) {
                        let http = CONTEXT.get().unwrap().http.clone();

                        let channels = member.guild_id.channels(http.clone()).await.unwrap();
                        let channel = channels.get(channel_id);

                        match channel {
                            Some(channel) => {
                                channel.send_message(http.clone(), CreateMessage::default()
                                    .content(format!("User {} assigned the honeypot role to themselves", member.user.id))
                                    .embed(CreateEmbed::default()
                                        .field("User", format!("{} / {}", member.display_name(), member.user.name), false)
                                    )
                                ).await.unwrap();
                            }
                            None => {}
                        }
                    }
                }
                else {
                    // They only have self-assigned roles so kick them.
                    let http = CONTEXT.get().unwrap().http.clone();
                    if let Some(channel_id) = SETTINGS.get().unwrap().log_channels.get(&member.guild_id) {
                        tracing::info!("User {} has a non-self-assignable role", member.user.id);

                        let channels = member.guild_id.channels(http.clone()).await.unwrap();
                        let channel = channels.get(channel_id);

                        match channel {
                            Some(channel) => {
                                channel.send_message(http.clone(), CreateMessage::default()
                                    .content(format!("User {} was kicked for assigning the honeypot role to themselves", member.user.id))
                                    .embed(CreateEmbed::default()
                                        .field("User", format!("{} / {}", member.display_name(), member.user.name), false)                                  )
                                ).await.unwrap();
                            }
                            None => {}
                        };
                        member.kick_with_reason(&http, "Kicked for antispam honeypot role").await.unwrap();
                    }
                }
            }
        }
        None => {}
    }
}
