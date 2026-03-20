use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use common::{
    config::Settings,
    events::{
        EventEnvelope, EventPayload, GuildAvailablePayload, GuildRemovedPayload,
        MemberJoinedPayload, MemberLeftPayload, MemberRolesUpdatedPayload, MessageCreatedPayload,
        ReactionAddedPayload, VoiceStateUpdatedPayload,
    },
    queue::{EventQueue, RedisEventQueue},
    store::{PostgresRawEventStore, RawEventStore},
};
use serenity::{
    Client,
    all::{
        GatewayIntents, Guild, GuildId, GuildMemberUpdateEvent, Member, Message, Reaction, Ready,
        UnavailableGuild, User, VoiceState,
    },
    async_trait,
    prelude::{Context as DiscordContext, EventHandler},
};
use sqlx::postgres::PgPoolOptions;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

struct Pipeline {
    queue: RedisEventQueue,
    store: PostgresRawEventStore,
}

impl Pipeline {
    async fn publish(&self, envelope: EventEnvelope) -> Result<()> {
        self.store.insert(&envelope).await?;
        let payload =
            serde_json::to_string(&envelope).context("failed to serialize queue event")?;
        self.queue.publish(envelope.stream_name(), &payload).await?;
        Ok(())
    }
}

struct Handler {
    pipeline: Arc<Pipeline>,
}

impl Handler {
    async fn dispatch(&self, envelope: EventEnvelope) {
        if let Err(error) = self.pipeline.publish(envelope).await {
            error!(?error, "failed to persist and enqueue event");
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: DiscordContext, ready: Ready) {
        info!(bot_user = %ready.user.name, "gateway connected");
    }

    async fn guild_create(&self, _: DiscordContext, guild: Guild, is_new: Option<bool>) {
        let member_count = i64::try_from(guild.member_count).unwrap_or(i64::MAX);
        let envelope = EventEnvelope::new(
            guild.id.to_string(),
            None,
            Some(guild.owner_id.to_string()),
            Utc::now(),
            EventPayload::GuildAvailable(GuildAvailablePayload {
                guild_id: guild.id.to_string(),
                name: guild.name.clone(),
                member_count,
                owner_id: guild.owner_id.to_string(),
                is_new: is_new.unwrap_or(false),
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn guild_delete(
        &self,
        _: DiscordContext,
        incomplete: UnavailableGuild,
        _: Option<Guild>,
    ) {
        let envelope = EventEnvelope::new(
            incomplete.id.to_string(),
            None,
            None,
            Utc::now(),
            EventPayload::GuildRemoved(GuildRemovedPayload {
                guild_id: incomplete.id.to_string(),
                is_unavailable: incomplete.unavailable,
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn guild_member_addition(&self, _: DiscordContext, member: Member) {
        let joined_at = member.joined_at.as_ref().map(timestamp_to_chrono);
        let envelope = EventEnvelope::new(
            member.guild_id.to_string(),
            None,
            Some(member.user.id.to_string()),
            joined_at.unwrap_or_else(Utc::now),
            EventPayload::MemberJoined(MemberJoinedPayload {
                member_id: member.user.id.to_string(),
                joined_at,
                is_pending: member.pending,
                role_ids: member
                    .roles
                    .iter()
                    .map(|role_id| role_id.to_string())
                    .collect(),
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn guild_member_removal(
        &self,
        _: DiscordContext,
        guild_id: GuildId,
        user: User,
        member_data_if_available: Option<Member>,
    ) {
        let envelope = EventEnvelope::new(
            guild_id.to_string(),
            None,
            Some(user.id.to_string()),
            Utc::now(),
            EventPayload::MemberLeft(MemberLeftPayload {
                member_id: user.id.to_string(),
                had_member_record: member_data_if_available.is_some(),
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn guild_member_update(
        &self,
        _: DiscordContext,
        old_if_available: Option<Member>,
        new: Option<Member>,
        event: GuildMemberUpdateEvent,
    ) {
        let previous_roles = old_if_available
            .as_ref()
            .map(|member| {
                member
                    .roles
                    .iter()
                    .map(|role_id| role_id.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let current_role_ids = new
            .as_ref()
            .map(|member| {
                member
                    .roles
                    .iter()
                    .map(|role_id| role_id.to_string())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| {
                event
                    .roles
                    .iter()
                    .map(|role_id| role_id.to_string())
                    .collect()
            });

        let added_role_ids = current_role_ids
            .iter()
            .filter(|role_id| !previous_roles.contains(role_id))
            .cloned()
            .collect::<Vec<_>>();
        let removed_role_ids = previous_roles
            .iter()
            .filter(|role_id| !current_role_ids.contains(role_id))
            .cloned()
            .collect::<Vec<_>>();

        if added_role_ids.is_empty() && removed_role_ids.is_empty() {
            return;
        }

        let envelope = EventEnvelope::new(
            event.guild_id.to_string(),
            None,
            Some(event.user.id.to_string()),
            Utc::now(),
            EventPayload::MemberRolesUpdated(MemberRolesUpdatedPayload {
                member_id: event.user.id.to_string(),
                added_role_ids,
                removed_role_ids,
                current_role_ids,
                is_pending: event.pending,
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn message(&self, _: DiscordContext, message: Message) {
        let Some(guild_id) = message.guild_id else {
            return;
        };

        let envelope = EventEnvelope::new(
            guild_id.to_string(),
            Some(message.channel_id.to_string()),
            Some(message.author.id.to_string()),
            timestamp_to_chrono(&message.timestamp),
            EventPayload::MessageCreated(MessageCreatedPayload {
                message_id: message.id.to_string(),
                author_id: message.author.id.to_string(),
                is_bot: message.author.bot,
                is_reply: message.referenced_message.is_some()
                    || message.message_reference.is_some(),
                attachment_count: i32::try_from(message.attachments.len()).unwrap_or(i32::MAX),
                content_length: i32::try_from(message.content.chars().count()).unwrap_or(i32::MAX),
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn reaction_add(&self, _: DiscordContext, reaction: Reaction) {
        let Some(guild_id) = reaction.guild_id else {
            return;
        };
        let Some(user_id) = reaction.user_id else {
            return;
        };

        let envelope = EventEnvelope::new(
            guild_id.to_string(),
            Some(reaction.channel_id.to_string()),
            Some(user_id.to_string()),
            Utc::now(),
            EventPayload::ReactionAdded(ReactionAddedPayload {
                message_id: reaction.message_id.to_string(),
                user_id: user_id.to_string(),
                emoji: reaction.emoji.to_string(),
            }),
        );

        self.dispatch(envelope).await;
    }

    async fn voice_state_update(
        &self,
        _: DiscordContext,
        old: Option<VoiceState>,
        new: VoiceState,
    ) {
        let Some(guild_id) = new.guild_id else {
            return;
        };

        let envelope = EventEnvelope::new(
            guild_id.to_string(),
            new.channel_id.map(|channel_id| channel_id.to_string()),
            Some(new.user_id.to_string()),
            Utc::now(),
            EventPayload::VoiceStateUpdated(VoiceStateUpdatedPayload {
                member_id: new.user_id.to_string(),
                old_channel_id: old.and_then(|state| state.channel_id.map(|id| id.to_string())),
                new_channel_id: new.channel_id.map(|id| id.to_string()),
            }),
        );

        self.dispatch(envelope).await;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let settings = Settings::from_env()?;
    init_tracing(&settings.rust_log);

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&settings.database_url)
        .await
        .context("failed to connect to postgres")?;

    let store = PostgresRawEventStore::new(pool);
    store.ensure_schema().await?;

    let queue = RedisEventQueue::new(&settings.redis_url)?;
    let handler = Handler {
        pipeline: Arc::new(Pipeline { queue, store }),
    };

    let mut intents = GatewayIntents::GUILDS
        | GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::GUILD_MESSAGE_REACTIONS
        | GatewayIntents::GUILD_VOICE_STATES;

    if settings.discord_enable_guild_members_intent {
        intents |= GatewayIntents::GUILD_MEMBERS;
    }

    let mut client = Client::builder(&settings.discord_token, intents)
        .event_handler(handler)
        .application_id(settings.discord_application_id.into())
        .await
        .context("failed to build serenity client")?;

    client.start_autosharded().await.context("gateway crashed")
}

fn init_tracing(rust_log: &str) {
    let filter = EnvFilter::try_new(rust_log).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

fn timestamp_to_chrono(timestamp: &serenity::model::Timestamp) -> DateTime<Utc> {
    DateTime::from_timestamp(timestamp.unix_timestamp(), 0).unwrap_or_else(Utc::now)
}
