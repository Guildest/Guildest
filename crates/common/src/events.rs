use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub event_id: Uuid,
    pub event_name: String,
    pub guild_id: String,
    pub channel_id: Option<String>,
    pub user_id: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub version: i32,
    pub payload: EventPayload,
}

impl EventEnvelope {
    pub fn new(
        guild_id: impl Into<String>,
        channel_id: Option<String>,
        user_id: Option<String>,
        occurred_at: DateTime<Utc>,
        payload: EventPayload,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4(),
            event_name: payload.kind().to_string(),
            guild_id: guild_id.into(),
            channel_id,
            user_id,
            occurred_at,
            received_at: Utc::now(),
            version: 1,
            payload,
        }
    }

    pub fn stream_name(&self) -> &'static str {
        self.payload.stream_name()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum EventPayload {
    GuildAvailable(GuildAvailablePayload),
    GuildRemoved(GuildRemovedPayload),
    MemberJoined(MemberJoinedPayload),
    MemberLeft(MemberLeftPayload),
    MemberRolesUpdated(MemberRolesUpdatedPayload),
    MessageCreated(MessageCreatedPayload),
    ReactionAdded(ReactionAddedPayload),
    VoiceStateUpdated(VoiceStateUpdatedPayload),
}

impl EventPayload {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::GuildAvailable(_) => "guild_available",
            Self::GuildRemoved(_) => "guild_removed",
            Self::MemberJoined(_) => "member_joined",
            Self::MemberLeft(_) => "member_left",
            Self::MemberRolesUpdated(_) => "member_roles_updated",
            Self::MessageCreated(_) => "message_created",
            Self::ReactionAdded(_) => "reaction_added",
            Self::VoiceStateUpdated(_) => "voice_state_updated",
        }
    }

    pub fn stream_name(&self) -> &'static str {
        match self {
            Self::GuildAvailable(_) | Self::GuildRemoved(_) => "events.guild",
            Self::MemberJoined(_) | Self::MemberLeft(_) | Self::MemberRolesUpdated(_) => {
                "events.member"
            }
            Self::MessageCreated(_) | Self::ReactionAdded(_) => "events.message",
            Self::VoiceStateUpdated(_) => "events.voice",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildAvailablePayload {
    pub guild_id: String,
    pub name: String,
    pub member_count: i64,
    pub owner_id: String,
    pub is_new: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuildRemovedPayload {
    pub guild_id: String,
    pub is_unavailable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberJoinedPayload {
    pub member_id: String,
    pub joined_at: Option<DateTime<Utc>>,
    pub is_pending: bool,
    pub role_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberLeftPayload {
    pub member_id: String,
    pub had_member_record: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberRolesUpdatedPayload {
    pub member_id: String,
    pub added_role_ids: Vec<String>,
    pub removed_role_ids: Vec<String>,
    pub current_role_ids: Vec<String>,
    pub is_pending: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageCreatedPayload {
    pub message_id: String,
    pub author_id: String,
    pub is_bot: bool,
    pub is_reply: bool,
    pub attachment_count: i32,
    pub content_length: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionAddedPayload {
    pub message_id: String,
    pub user_id: String,
    pub emoji: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceStateUpdatedPayload {
    pub member_id: String,
    pub old_channel_id: Option<String>,
    pub new_channel_id: Option<String>,
}
