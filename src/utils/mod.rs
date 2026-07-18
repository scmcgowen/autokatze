use poise::serenity_prelude as serenity;
use poise::serenity_prelude::{Member, Permissions};

use crate::CONTEXT;

// Miscellaneous utility functions for the bot

pub async fn is_staff(member: &Member, ctx: &serenity::Context) -> bool {
    if let Some(guild) = member.guild_id.to_guild_cached(ctx) {
        if member.user.id == guild.owner_id {
            return true;
        }
    }
    for role_id_ref in &member.roles {
        let role_id = *role_id_ref;
        let role = member
            .guild_id
            .role(CONTEXT.get().unwrap(), role_id)
            .await
            .ok();

        if let Some(r) = role {
            // If the user has any of these permissions, they're likely at least a moderator
            if r.permissions.contains(Permissions::ADMINISTRATOR)
                || r.permissions.contains(Permissions::KICK_MEMBERS)
                || r.permissions.contains(Permissions::BAN_MEMBERS)
                || r.permissions.contains(Permissions::MANAGE_CHANNELS)
                || r.permissions.contains(Permissions::MANAGE_GUILD)
                || r.permissions.contains(Permissions::MANAGE_MESSAGES)
                || r.permissions.contains(Permissions::MANAGE_ROLES)
            {
                return true;
            }
        }
    }
    false
}
