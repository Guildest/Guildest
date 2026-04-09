# Guildest AI Agent Pivot

Guildest should move from "Discord analytics with some AI" to an AI community operator for Discord. The core product promise is:

> Guildest tells you what your Discord community needs, why it matters, and what to do next.

The statistics still matter, but they become evidence. The main product surface should be synthesis, recommendations, alerts, drafts, and eventually approved automation.

## Product Positioning

Guildest is not a general chatbot in Discord. It should feel closer to a Codex-style agent for a server: it has access to the real community context, understands what has happened, and can give accurate, grounded next steps.

The agent should treat a Discord server the way Codex treats a repository. A repository has files, diffs, issues, commits, and history. A Discord server has channels, threads, messages, reactions, roles, users, questions, complaints, launches, and feedback loops. Guildest should index that living context so it can reason from evidence rather than behave like a generic chat assistant.

Primary framing:

- Eyes and ears for your Discord server
- AI community operator for founders and community teams
- Real-time community intelligence for startups
- Product feedback intelligence from Discord conversations
- Founder-ready pulses and briefings from messy community activity

Do not lead with "analytics bot." Lead with the operational outcome: knowing what users are saying, what is changing, and what actions should happen next.

## Ideal Customers

Initial customers:

- Startups using Discord as a customer community
- AI, gaming, developer, crypto, creator, and SaaS communities
- Founders who personally monitor Discord but are starting to miss signal
- Community managers who need summaries, follow-ups, and evidence
- Small teams using Discord as an informal product feedback system

Enterprise path:

- Larger support communities
- Product teams that need feedback categorization
- Companies with audit, retention, and privacy requirements
- Teams that want private deployment or stronger data controls

## User-Facing Surfaces

### Live Pulse

The primary loop should be near real-time, not weekly. Startups do not want to discover pricing friction, launch confusion, or an angry support thread a week later. They need the useful version of "what is happening right now?"

Live Pulse should be a compact, continuously refreshed admin view:

- What changed in the last hour
- What needs attention now
- Which conversations are unresolved
- Which themes are repeating
- Which users look confused, frustrated, excited, or high-intent
- What action Guildest recommends next

Live Pulse should feel like the server is being watched by an operator, not like a report generator.

### Real-Time Alerts

Real-time should be selective. Guildest should not notify admins for every mildly interesting message. It should alert only when the signal is urgent, actionable, and likely to decay if ignored.

Example alert triggers:

- Multiple users mention the same bug, pricing objection, or onboarding blocker in a short window.
- A support question from a likely customer or high-value member goes unanswered.
- A launch, announcement, or release thread starts receiving negative replies.
- A frustrated user receives no useful response.
- A feature request repeats across channels.
- A moderation or safety risk begins to form.

The alert should include the reason, evidence, and suggested response. Alerts should be rate-limited and configurable per guild.

### Daily Briefing

A recurring DM or dashboard briefing for admins and owners.

It should answer:

- What changed today?
- What feedback or complaints are repeating?
- Which conversations need a response?
- Which users seem confused, frustrated, excited, or at risk?
- What should the team do next?

The briefing should include concise evidence. It should not merely say "sentiment is negative." It should say what caused the signal and point to the relevant channels or messages when permissions allow.

### Weekly Strategy Report

Weekly reports should exist, but they should not be the core product loop. They are for trends and strategy, not urgent operations.

A weekly report should answer:

- What changed this week compared with last week?
- Which themes are growing?
- Which concerns are cooling down?
- What did users love?
- What did users repeatedly ask for?
- Which product/community bets should the team prioritize?

Recommended cadence hierarchy:

- Real-time alerts for urgency
- Hourly Live Pulse for awareness
- Daily briefing for operations
- Weekly report for strategy

### Curated Feedback Dashboard

The dashboard should shift from metrics-first to intelligence-first.

Core sections:

- Live Pulse
- Needs attention
- Real-time alerts
- Today's briefing
- Community health
- Recommended actions
- Evidence
- Ask Guildest

Adaptive sections:

- Product feedback
- Pricing and conversion friction
- Bugs and support issues
- Feature requests
- Praise and advocacy
- Verification and onboarding
- Events and rituals
- Docs gaps
- Moderation risks

Existing stats pages can remain as supporting tabs.

### Adaptive Dashboard

Guildest should not show every guild the same dashboard. The agent should first understand what kind of server it has joined, then show the modules that fit that community.

The default dashboard should stay simple:

- Live Pulse
- Needs Attention
- Recommended Actions
- Ask Guildest

Specialized modules should appear only when the Server Map says they are relevant:

- Product Feedback
- Pricing Friction
- Support Queue
- Verification Funnel
- New Member Onboarding
- Community Health
- Moderation Signals
- Events
- Creator Insights
- Docs Gaps
- Power Users

Examples:

- A startup/product Discord should see bugs, feature requests, pricing objections, support pain, launch reactions, and product recommendations.
- A creator community should see engagement, fan questions, content requests, event ideas, and VIP members.
- A gaming community should see event attendance, LFG activity, moderation risks, onboarding, and channel health.
- An open-source/developer community should see docs gaps, repeated technical issues, unanswered help threads, contributor activity, and integration questions.
- A general community or discourse server should see activity health, unresolved conversations, newcomer experience, moderation risks, and community rituals, not product-feedback modules.

The UI principle is:

> Show what this server needs, not everything Guildest can technically measure.

### First-Run Mapping Flow

The first user experience should feel like Guildest joined the server, looked around, and understood its shape.

Recommended flow:

1. User invites Guildest.
2. User connects the dashboard.
3. Guildest scans visible server structure: channels, roles, known bots, permissions, activity, threads, and recent metadata.
4. Guildest generates an initial Server Map.
5. Dashboard says "Here is what I found."
6. Owner confirms, edits, or ignores inferred details.
7. Dashboard enables only the modules that match the confirmed Server Map.

The setup should ask at most a few lightweight questions:

- What is this server mainly for?
- What should Guildest help with first?
- How often should Guildest update you?

These questions should be optional. Guildest should infer a starting profile from evidence like channel names, role names, bot behavior, and conversation patterns.

Example confirmation copy:

> Guildest mapped this as a gated product/support community. New members appear to start in #rules and #verify, then receive access after a verification bot assigns a role. Most support questions happen in #help, and product feedback appears in #ideas.

Possible actions:

- Looks right
- Edit server type
- Change monitored channels
- Ignore verification flow

### Ask Guildest

An admin-only question interface over server context.

Example questions:

- What are people saying about pricing?
- What confused new users this week?
- Which channels need moderator attention?
- What feature requests keep coming up?
- Which users should we follow up with?
- What should we announce next?

This should use retrieval over stored observations, summaries, and metrics rather than sending entire server history to an LLM.

### Action Drafts

Guildest should draft actions before it performs actions.

Initial draft types:

- Reply to a thread
- Follow-up DM to a user
- Announcement
- FAQ entry
- Support response
- Product feedback summary
- Daily or weekly report

The first version should require human approval before sending anything.

## Product Modes

### Advisor Mode

Guildest observes, summarizes, recommends, and drafts. It does not post or DM users automatically.

This should be the default mode.

### Approval Mode

Guildest can prepare actions and ask an admin to approve them.

Example:

> Three users asked the same onboarding question. Approve this FAQ reply in #help?

### Automation Mode

Guildest can perform low-risk actions based on rules and permissions.

Examples:

- Create weekly reports
- Flag unanswered questions
- Draft but not send product feedback exports
- Send approved recurring reminders

Automation mode should come later, after the agent has earned trust.

## Server Map

The agent needs a durable understanding of each server before it can recommend useful work. Internally, this should be called the Server Map.

The Server Map should describe:

- Server type
- Owner goals
- Important channels
- Important roles
- Known bots
- Entry and verification flow
- Active support areas
- Feedback areas
- Announcement areas
- Staff or moderator areas visible to Guildest
- Channels that should be ignored
- Enabled dashboard modules
- Confidence and evidence for each inference

The Server Map should be explainable and editable. The owner should be able to correct Guildest when it guesses wrong.

### Server Types

Initial server type categories:

- `product_support`
- `creator_community`
- `gaming_community`
- `developer_community`
- `education_community`
- `paid_private_community`
- `general_discourse`
- `internal_team`
- `unknown`

Server type should not be a permanent label. Guildest should be able to revise it when the owner edits the map or when the server evolves.

### Gate Detection

Many Discord servers have a gate before normal participation. Guildest should detect and explain that gate because it changes what the owner needs to see.

Suggested gate types:

- `none` - users can participate immediately.
- `discord_native` - Discord community onboarding, membership screening, or rules acceptance.
- `role_gate` - users need a role before channels unlock.
- `bot_verification` - a bot handles buttons, CAPTCHA, reaction roles, commands, or verification flows.
- `application_gate` - users answer questions or wait for approval.
- `payment_gate` - access is tied to paid status through services like Stripe, Patreon, Whop, LaunchPass, or similar tools.
- `manual_gate` - staff manually approves users or assigns roles.
- `unknown` - Guildest sees signs of a gate but cannot determine the flow.

Gate detection should produce a human-readable explanation:

> Guildest thinks this server uses a bot verification gate in #verify. Users appear to receive the Member role after completing it.

Gate-aware modules should appear only when relevant:

- Verification Funnel
- Gate Dropoff
- New Member Activation
- Rules Acceptance
- Application Queue
- Paid Access Health

If a server has no gate, the dashboard should not waste space on gate analytics.

### Server Map Evidence

The Server Map should be grounded in evidence:

- Channel names and topics
- Role names
- Permission patterns
- Known bot user IDs or bot names
- Message and reaction patterns
- Join-to-first-message timing
- Role assignment timing
- Threads and support-channel behavior
- Owner corrections

Each inference should carry a confidence level and evidence pointers. Low-confidence inferences should be presented as suggestions, not facts.

## Architecture Fit

The existing Guildest architecture already supports this pivot well:

```text
Discord Gateway
    |
    v
raw_events + Redis streams
    |
    v
analytics workers
    |
    v
fact tables + rollups
    |
    v
dashboard/API
```

The AI agent should extend the pipeline rather than replace it:

```text
Discord Gateway
    |
    v
raw event metadata
    |
    +--> analytics workers
    |
    +--> AI observation jobs
             |
             v
       redaction + classification
             |
             v
       feedback items + themes + alerts
             |
             v
       briefings + recommendations + drafts
             |
             v
       dashboard + owner DM + Ask Guildest
```

The gateway should stay thin. It should capture and enqueue only what the configured guild has opted into. Heavy AI work belongs in workers.

## Content Capture

The current system tracks message metadata, not message text. The AI pivot needs an explicit opt-in content pipeline.

Important rules:

- Message content analysis must be disabled by default.
- A server admin must opt in.
- Admins should choose monitored and excluded channels.
- DMs should not be captured.
- Private or sensitive channels should be excluded by default where possible.
- Stored content should have a retention window.
- AI outputs should keep evidence small and relevant.
- The product should explain what is stored and why.

Recommended storage approach:

- Keep raw events metadata-oriented.
- Store AI-ready message text separately from raw event metadata.
- Redact before any queue, database, embedding, or LLM boundary.
- Store derived feedback items and summaries longer than redacted message text when the retention policy allows it.
- Make retention configurable by plan.

### Privacy Boundary And Redaction Pipeline

Redaction in this context means stripping identifiable personal information and secrets from message text before it crosses a durable boundary. It does not mean replacing message content with a hash.

The default privacy contract should be:

- Raw message content is handled only in process memory.
- Raw message content is never written to Postgres.
- Raw message content is never written to Redis streams.
- Raw message content is never logged.
- Raw message content is never sent to an LLM provider.
- Redacted content is the only text allowed in `ai_message_observations.content_redacted`.
- If redaction fails, the observation is stored without content and no downstream AI text processing runs for that message.
- Admins can disable AI and purge stored AI observations, embeddings, evidence snippets, recommendations, and briefings for a guild.
- HMAC secrets should be managed through deployment secrets, not application config committed to the repo.

The first implementation should run deterministic redaction in the gateway before publishing an AI observation job. The gateway should still stay thin: it only checks AI settings, redacts eligible message content, computes a keyed fingerprint, and publishes the redacted observation reference. Workers handle classification, clustering, alerting, summaries, recommendations, and model calls.

If a future architecture needs worker-side redaction, raw content must be encrypted before queueing, never logged, short-lived, and covered by a separate threat model. That should not be the MVP path.

Redaction should remove or mask:

- Email addresses
- Phone numbers
- Discord user mentions and tags when they identify a person outside useful evidence
- URLs containing user tokens, credentials, invite codes, or private resources
- API keys, OAuth tokens, session tokens, auth headers, and recovery phrases
- Wallet addresses, payment details, order IDs, and license keys
- IP addresses and exact physical addresses
- Dates of birth and government-style identifiers
- File paths that include local usernames
- Private repository URLs and internal hostnames
- Names following patterns like "my name is X" or "I'm X"

Redaction should not remove:

- Product names, feature names, or company names
- Technical terms, error messages, or stack traces after secrets and personal identifiers inside them are removed
- Sentiment-bearing language that is not personally identifying

The original message text should never be stored in Postgres. Only the redacted form enters `content_redacted`. If redaction fails for any reason, the field should be left null and the observation stored without content.

Use a keyed HMAC fingerprint for deduplication instead of a plain hash. Plain SHA-256 is vulnerable to guessing attacks against short Discord messages. Treat fingerprints as sensitive metadata and delete them on the same retention schedule as their observations unless there is a clear product reason not to.

Redaction should be versioned. Every stored observation should record whether redaction succeeded, which redaction version produced the text, and whether content was omitted because capture was disabled or redaction failed.

## Proposed Data Model

### ai_guild_settings

Per-guild AI configuration.

- `guild_id TEXT PRIMARY KEY`
- `ai_enabled BOOLEAN NOT NULL DEFAULT FALSE`
- `advisor_mode_enabled BOOLEAN NOT NULL DEFAULT TRUE`
- `approval_required BOOLEAN NOT NULL DEFAULT TRUE`
- `owner_dm_enabled BOOLEAN NOT NULL DEFAULT FALSE`
- `live_pulse_enabled BOOLEAN NOT NULL DEFAULT TRUE`
- `live_pulse_interval_minutes INTEGER NOT NULL DEFAULT 60`
- `real_time_alerts_enabled BOOLEAN NOT NULL DEFAULT TRUE`
- `daily_briefing_enabled BOOLEAN NOT NULL DEFAULT TRUE`
- `weekly_report_enabled BOOLEAN NOT NULL DEFAULT TRUE`
- `retention_days INTEGER NOT NULL DEFAULT 30`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- `updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`

### ai_channel_settings

Channel-level monitoring controls.

- `guild_id TEXT NOT NULL`
- `channel_id TEXT NOT NULL`
- `monitoring_enabled BOOLEAN NOT NULL DEFAULT TRUE`
- `content_analysis_enabled BOOLEAN NOT NULL DEFAULT FALSE`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- `updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- primary key: `guild_id, channel_id`

### ai_server_profiles

Durable Server Map profile for a guild.

- `guild_id TEXT PRIMARY KEY`
- `server_type TEXT NOT NULL DEFAULT 'unknown'`
- `server_type_confidence REAL NOT NULL DEFAULT 0`
- `owner_goals JSONB NOT NULL DEFAULT '[]'::jsonb`
- `enabled_modules JSONB NOT NULL DEFAULT '[]'::jsonb`
- `ignored_channel_ids JSONB NOT NULL DEFAULT '[]'::jsonb`
- `known_bot_user_ids JSONB NOT NULL DEFAULT '[]'::jsonb`
- `summary TEXT NULL`
- `evidence JSONB NOT NULL DEFAULT '[]'::jsonb`
- `last_mapped_at TIMESTAMPTZ NULL`
- `confirmed_at TIMESTAMPTZ NULL`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- `updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`

### ai_channel_profiles

Agent understanding of what each channel is for.

- `guild_id TEXT NOT NULL`
- `channel_id TEXT NOT NULL`
- `channel_purpose TEXT NOT NULL DEFAULT 'unknown'`
- `purpose_confidence REAL NOT NULL DEFAULT 0`
- `summary TEXT NULL`
- `recurring_topics JSONB NOT NULL DEFAULT '[]'::jsonb`
- `is_gate_channel BOOLEAN NOT NULL DEFAULT FALSE`
- `is_support_channel BOOLEAN NOT NULL DEFAULT FALSE`
- `is_feedback_channel BOOLEAN NOT NULL DEFAULT FALSE`
- `is_announcement_channel BOOLEAN NOT NULL DEFAULT FALSE`
- `is_social_channel BOOLEAN NOT NULL DEFAULT FALSE`
- `last_profiled_at TIMESTAMPTZ NULL`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- `updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- primary key: `guild_id, channel_id`

### ai_gate_profiles

Agent understanding of the server entry flow.

- `guild_id TEXT PRIMARY KEY`
- `gate_type TEXT NOT NULL DEFAULT 'unknown'`
- `confidence REAL NOT NULL DEFAULT 0`
- `gate_channel_id TEXT NULL`
- `gate_bot_user_id TEXT NULL`
- `required_role_ids JSONB NOT NULL DEFAULT '[]'::jsonb`
- `flow_summary TEXT NULL`
- `evidence JSONB NOT NULL DEFAULT '[]'::jsonb`
- `confirmed_at TIMESTAMPTZ NULL`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- `updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`

### ai_message_observations

AI-safe message records for retrieval and classification.

- `id BIGSERIAL PRIMARY KEY`
- `guild_id TEXT NOT NULL`
- `channel_id TEXT NOT NULL`
- `message_id TEXT NOT NULL`
- `author_id TEXT NOT NULL`
- `occurred_at TIMESTAMPTZ NOT NULL`
- `content_redacted TEXT NULL` - PII/secret-stripped message text; null if redaction failed or content capture is disabled
- `content_fingerprint TEXT NULL` - HMAC-SHA256 fingerprint of the original message text for deduplication; original text is never stored
- `redaction_status TEXT NOT NULL DEFAULT 'not_captured'`
- `redaction_version TEXT NULL`
- `language TEXT NULL`
- `is_question BOOLEAN NOT NULL DEFAULT FALSE`
- `is_feedback BOOLEAN NOT NULL DEFAULT FALSE`
- `is_support_request BOOLEAN NOT NULL DEFAULT FALSE`
- `sentiment TEXT NULL`
- `urgency TEXT NULL`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- unique: `guild_id, message_id`

Suggested `redaction_status` values:

- `not_captured`
- `redacted`
- `failed`
- `omitted_by_policy`

### ai_memory_items

Compacted agent memory for retrieval and future reasoning.

- `id UUID PRIMARY KEY`
- `guild_id TEXT NOT NULL`
- `memory_type TEXT NOT NULL`
- `scope_kind TEXT NOT NULL`
- `scope_id TEXT NULL`
- `title TEXT NOT NULL`
- `body TEXT NOT NULL`
- `confidence REAL NOT NULL DEFAULT 0`
- `source_ids JSONB NOT NULL DEFAULT '[]'::jsonb`
- `expires_at TIMESTAMPTZ NULL`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- `updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`

Suggested `memory_type` values:

- `server_profile`
- `channel_summary`
- `gate_summary`
- `theme_summary`
- `user_role_note`
- `policy_instruction`
- `action_history`

### ai_feedback_items

Durable extracted signals.

- `id UUID PRIMARY KEY`
- `guild_id TEXT NOT NULL`
- `channel_id TEXT NULL`
- `source_message_id TEXT NULL`
- `author_id TEXT NULL`
- `category TEXT NOT NULL`
- `title TEXT NOT NULL`
- `summary TEXT NOT NULL`
- `evidence TEXT NULL`
- `sentiment TEXT NULL`
- `urgency TEXT NOT NULL DEFAULT 'normal'`
- `status TEXT NOT NULL DEFAULT 'open'`
- `first_seen_at TIMESTAMPTZ NOT NULL`
- `last_seen_at TIMESTAMPTZ NOT NULL`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`

Suggested categories:

- `bug`
- `feature_request`
- `pricing_objection`
- `onboarding_confusion`
- `support_issue`
- `churn_risk`
- `praise`
- `moderation_risk`
- `community_health`

### ai_theme_snapshots

Aggregated themes over a time window.

- `id UUID PRIMARY KEY`
- `guild_id TEXT NOT NULL`
- `window_start TIMESTAMPTZ NOT NULL`
- `window_end TIMESTAMPTZ NOT NULL`
- `theme TEXT NOT NULL`
- `summary TEXT NOT NULL`
- `item_count INTEGER NOT NULL`
- `sentiment TEXT NULL`
- `trend TEXT NULL`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`

### ai_recommendations

Actionable recommendations.

- `id UUID PRIMARY KEY`
- `guild_id TEXT NOT NULL`
- `title TEXT NOT NULL`
- `body TEXT NOT NULL`
- `rationale TEXT NOT NULL`
- `recommended_action TEXT NOT NULL`
- `status TEXT NOT NULL DEFAULT 'open'`
- `priority TEXT NOT NULL DEFAULT 'medium'`
- `source_kind TEXT NULL`
- `source_id TEXT NULL`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- `resolved_at TIMESTAMPTZ NULL`

### ai_alerts

High-signal events that should be shown immediately or delivered to an admin.

- `id UUID PRIMARY KEY`
- `guild_id TEXT NOT NULL`
- `alert_type TEXT NOT NULL`
- `title TEXT NOT NULL`
- `body TEXT NOT NULL`
- `rationale TEXT NOT NULL`
- `priority TEXT NOT NULL DEFAULT 'medium'`
- `source_kind TEXT NULL`
- `source_id TEXT NULL`
- `status TEXT NOT NULL DEFAULT 'open'`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- `acknowledged_at TIMESTAMPTZ NULL`

### ai_briefings

Generated owner/admin pulses, briefings, and reports.

- `id UUID PRIMARY KEY`
- `guild_id TEXT NOT NULL`
- `briefing_type TEXT NOT NULL`
- `period_start TIMESTAMPTZ NOT NULL`
- `period_end TIMESTAMPTZ NOT NULL`
- `title TEXT NOT NULL`
- `summary TEXT NOT NULL`
- `highlights JSONB NOT NULL`
- `risks JSONB NOT NULL`
- `recommended_actions JSONB NOT NULL`
- `evidence JSONB NOT NULL`
- `delivery_status TEXT NOT NULL DEFAULT 'draft'`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- `delivered_at TIMESTAMPTZ NULL`

Suggested `briefing_type` values:

- `live_pulse`
- `daily_briefing`
- `weekly_report`

### ai_action_drafts

Drafts awaiting admin approval.

- `id UUID PRIMARY KEY`
- `guild_id TEXT NOT NULL`
- `action_type TEXT NOT NULL`
- `target_channel_id TEXT NULL`
- `target_user_id TEXT NULL`
- `title TEXT NOT NULL`
- `body TEXT NOT NULL`
- `approval_status TEXT NOT NULL DEFAULT 'pending'`
- `created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()`
- `approved_at TIMESTAMPTZ NULL`
- `executed_at TIMESTAMPTZ NULL`

## Queue Design

Add AI-specific streams:

- `events.ai_observation`
- `jobs.ai_classify`
- `jobs.ai_alert_eval`
- `jobs.ai_live_pulse`
- `jobs.ai_theme_snapshot`
- `jobs.ai_briefing`
- `jobs.ai_action_draft`

The gateway should not call LLMs. It should publish references to persisted events or AI-safe message observations.

Recommended job flow:

1. `message_created` arrives.
2. Gateway checks whether the guild and channel have AI content capture enabled.
3. Gateway persists raw event metadata as it does today, without message content.
4. Gateway redacts eligible message content in memory.
5. Gateway stores or queues only an AI-safe observation containing redacted content, redaction metadata, and a keyed fingerprint.
6. Worker classifies observations in batches.
7. Worker evaluates alert thresholds for high-signal events.
8. Worker updates feedback items, recommendations, and theme snapshots.
9. Hourly job creates or refreshes Live Pulse.
10. Daily job creates operational briefings.
11. Weekly job creates strategic reports.
12. API serves AI outputs to the dashboard.
13. Optional delivery job sends owner/admin DMs.

## Continuous Agent Loop

Guildest should work continuously, but the expensive LLM should not be activated for every message. The agent loop should behave like an indexing and reasoning system:

1. Always ingest eligible events.
2. Run cheap filters and metadata heuristics continuously.
3. Batch message classification.
4. Trigger stronger AI only when a threshold is crossed, enough context accumulates, a scheduled pulse/briefing is due, or an admin asks a question.
5. Maintain rolling memory by channel, thread, user, and theme.
6. Retrieve relevant context before synthesis.
7. Store durable outputs so later calls build on previous work.

This makes Guildest feel present without turning every Discord message into a model call.

Suggested trigger types:

- Time trigger: refresh Live Pulse every 60 minutes by default.
- Volume trigger: analyze after N eligible messages in a guild or channel.
- Topic trigger: analyze immediately when pricing, bugs, churn risk, safety, or launch-related phrases appear.
- Silence trigger: alert when an important question remains unanswered.
- Spike trigger: alert when a topic or sentiment changes unusually fast.
- Manual trigger: run Ask Guildest when an admin queries the server.

## Agent Memory And Compaction

Guildest needs memory, but not unbounded raw history. The agent should compact Discord activity into progressively smaller, more useful layers:

```text
eligible messages
    -> redacted observations
    -> channel and thread summaries
    -> feedback items and themes
    -> pulses, briefings, and reports
    -> long-term server memory
```

Memory should be layered:

- Server Profile Memory: server type, owner goals, gate type, enabled modules, ignored channels, known bots, and durable owner corrections.
- Channel Memory: what each channel is for, recurring topics, typical response patterns, and whether it is support, feedback, social, announcement, gate, or staff-like.
- Gate Memory: verification path, required roles, likely gate bot, dropoff signals, and onboarding friction.
- Theme Memory: recurring subjects like pricing confusion, docs gaps, event requests, bugs, moderation issues, or onboarding problems.
- User/Role Memory: privacy-aware notes about likely staff, helpful members, high-intent users, frustrated users, and new users needing help.
- Action Memory: recommendations, drafts, approvals, rejections, sent messages, ignored alerts, and resolved issues.
- Policy Memory: owner instructions such as "do not monitor #off-topic", "alert only for support", or "never DM users automatically".

Compaction rules:

- Redacted observations can expire quickly based on guild retention settings.
- Embeddings, evidence snippets, and fingerprints should expire with observations unless explicitly retained by policy.
- Channel summaries can live longer than individual observations.
- Owner-confirmed Server Map facts can live until changed.
- Generated briefings and action history can live according to audit/retention settings.
- Ask Guildest should retrieve from compacted summaries first, then recent observations only when needed.

This keeps the agent useful, affordable, and explainable.

## Internal Analyzers

Users should see one agent: Guildest. Internally, the system can use specialized analyzers, but the UI should not expose confusing "sub-agent" language.

Potential internal analyzers:

- Server Mapper: infers server type, channel purposes, important roles, and enabled modules.
- Gate Analyst: understands verification, onboarding, approval, and role-gated flows.
- Support Watcher: finds unanswered questions, repeated issues, and support bottlenecks.
- Feedback Analyst: extracts bugs, feature requests, objections, praise, and product signals.
- Community Health Analyst: watches activity, retention, dead channels, rituals, and engagement quality.
- Moderation Analyst: watches conflict, raids, safety spikes, and moderation risks.
- Briefing Writer: turns signals into owner-readable pulses, briefings, and reports.
- Action Drafter: drafts replies, announcements, FAQs, and follow-up messages.

The orchestrator should decide which analyzers run based on the Server Map, recent activity, and enabled modules. For example, a general discourse server should not run heavy product-feedback analysis unless the owner enables it or the server starts showing product-like channels and conversations.

## API Surface

Initial dashboard endpoints:

- `GET /v1/dashboard/guilds/{guild_id}/ai/settings`
- `PUT /v1/dashboard/guilds/{guild_id}/ai/settings`
- `GET /v1/dashboard/guilds/{guild_id}/ai/server-map`
- `PUT /v1/dashboard/guilds/{guild_id}/ai/server-map`
- `GET /v1/dashboard/guilds/{guild_id}/ai/live-pulse`
- `GET /v1/dashboard/guilds/{guild_id}/ai/alerts`
- `GET /v1/dashboard/guilds/{guild_id}/ai/briefing`
- `GET /v1/dashboard/guilds/{guild_id}/ai/reports/weekly`
- `GET /v1/dashboard/guilds/{guild_id}/ai/feedback`
- `GET /v1/dashboard/guilds/{guild_id}/ai/recommendations`
- `POST /v1/dashboard/guilds/{guild_id}/ai/ask`
- `POST /v1/dashboard/guilds/{guild_id}/ai/action-drafts/{draft_id}/approve`
- `POST /v1/dashboard/guilds/{guild_id}/ai/action-drafts/{draft_id}/reject`

All endpoints should reuse the existing dashboard access checks.

## Frontend Plan

Add a new dashboard route:

- `/dashboard/ai`

Suggested page order:

1. Server Map confirmation when the map is new, low-confidence, or changed materially
2. Live Pulse
3. Needs attention
4. Real-time alerts
5. Recommended actions
6. Today's briefing
7. Adaptive modules based on server type and gate type
8. Evidence
9. Ask Guildest
10. Settings

The AI page should become the primary dashboard destination once the feature exists. Current analytics pages should become supporting pages.

## Prompt Contracts

Each AI task should have a strict output schema. Avoid loose natural-language-only outputs.

Recommended contracts:

- `infer_server_map`
- `infer_gate_profile`
- `summarize_channel`
- `classify_message`
- `extract_feedback_item`
- `evaluate_alert`
- `generate_live_pulse`
- `cluster_feedback_items`
- `generate_briefing`
- `generate_weekly_report`
- `generate_recommendations`
- `draft_action`
- `answer_admin_question`

Every generated answer should include:

- output type
- confidence
- supporting observation IDs
- recommended action if applicable
- short rationale

## Retrieval Strategy

Do not send full server history into the model.

Use a staged approach:

1. Store observations.
2. Classify and summarize incrementally.
3. Retrieve only relevant observations, feedback items, metrics, and summaries for each AI call.
4. Prefer smaller local summaries for routine jobs.
5. Use stronger models only for synthesis, recommendations, and Ask Guildest.

This keeps costs and latency manageable.

## Cost Controls

Cost levers:

- Batch classification
- Gate real-time alerts behind high-signal thresholds
- Cache generated theme snapshots
- Summarize incrementally by channel and time window
- Use cheaper models for classification
- Use stronger models only for final synthesis
- Limit Ask Guildest retrieval windows by default
- Rate-limit admin questions by plan
- Offer Live Pulse, daily briefing, and weekly report cadence controls
- Store embeddings only for AI-enabled guilds

Pricing should be based primarily on:

- AI-enabled message volume
- retention window
- Live Pulse interval
- alert volume
- briefing and report cadence
- number of admin seats
- automation level

## Trust, Privacy, And Safety

This pivot only works if server owners trust the agent.

Product requirements:

- AI disabled by default
- Clear opt-in flow
- Channel allowlist or blocklist
- Data retention controls
- Evidence controls
- Admin-only AI views
- Audit log for generated and executed actions
- Approval before posting
- No DM ingestion by default
- No raw content in Postgres, Redis, logs, embeddings, or external LLM calls
- Redaction before classification, embeddings, summaries, alerts, and Ask Guildest
- HMAC fingerprints instead of plain content hashes
- Guild-level AI data purge
- Easy disable switch

The product should make privacy feel like part of the value, not a buried setting.

## Implementation Roadmap

### Phase 0: Product Reframe

- Update product copy from analytics-first to AI-agent-first.
- Add this planning document.
- Decide the first customer story: founder/community owner Live Pulse.

### Phase 1: AI Settings And Content Capture

- Add `ai_guild_settings`.
- Add `ai_channel_settings`.
- Add `ai_server_profiles`.
- Add `ai_channel_profiles`.
- Add `ai_gate_profiles`.
- Add first-run Server Map inference from channel names, role names, known bots, permissions, and recent metadata.
- Add Server Map confirmation UI.
- Add admin dashboard settings UI.
- Add Message Content Intent documentation.
- Add gated content capture for AI-enabled guilds only.
- Add deterministic gateway-side redaction.
- Store redacted message observations.
- Store HMAC content fingerprints for deduplication.
- Add retention cleanup job.

### Phase 2: Classification And Feedback Extraction

- Add AI worker job types.
- Add compacted channel summaries and memory items.
- Classify messages into questions, feedback, support, praise, risks, and sentiment.
- Extract feedback items.
- Add feedback API endpoint.
- Add dashboard feedback view.

### Phase 3: Live Pulse And Alerts

- Generate hourly Live Pulse snapshots by default.
- Add Live Pulse dashboard.
- Select visible modules based on Server Map and owner overrides.
- Add high-signal real-time alert evaluation.
- Add alert dashboard and acknowledgement flow.
- Include evidence and recommended actions.

### Phase 4: Briefings And Reports

- Generate daily operational briefings.
- Generate weekly strategy reports.
- Add briefing and report dashboard sections.
- Add owner/admin DM delivery.
- Include evidence and recommended actions.
- Add delivery status and audit history.

### Phase 5: Ask Guildest

- Add retrieval over observations, feedback items, metrics, and briefings.
- Add admin-only Q&A endpoint.
- Add dashboard Ask Guildest UI.
- Return cited evidence.

### Phase 6: Action Drafts

- Generate draft replies, announcements, FAQ entries, and follow-up DMs.
- Add approval and rejection flow.
- Add action audit log.
- Allow approved Discord posting.

### Phase 7: Controlled Automation

- Add rules for low-risk recurring actions.
- Add per-action permissions.
- Add safety limits and rollback paths.
- Add enterprise controls.

## First Build Slice

The smallest useful slice should be:

1. Add AI settings tables.
2. Add Server Map tables and a deterministic first-pass mapper.
3. Add Server Map confirmation UI.
4. Add AI observation storage with content disabled by default.
5. Add a fixture or local-only path for testing AI observations without real Discord content.
6. Add a worker function that turns recent observations into a Live Pulse using a stubbed provider.
7. Add `GET /v1/dashboard/guilds/{guild_id}/ai/server-map`.
8. Add `GET /v1/dashboard/guilds/{guild_id}/ai/live-pulse`.
9. Add `/dashboard/ai` with Server Map confirmation, a Live Pulse panel, and recommended actions.

The first slice can use a deterministic mock AI provider. That lets the product and pipeline ship before choosing final model routing.

## Open Decisions

- Which model provider should power production summaries?
- Should Guildest store embeddings in Postgres with `pgvector`, a hosted vector database, or no vector store at first?
- Should message content be stored after redaction, or should only derived observations persist?
- What is the default retention window for each plan?
- What should the Server Map feature be called in the product UI?
- Which modules should be available for each initial server type?
- How much evidence should Guildest show when explaining a gate inference?
- Should owner-confirmed Server Map facts expire, or persist until manually changed?
- Should owner DMs be enabled by default after opt-in, or require a second explicit toggle?
- What Discord permissions are required for posting approved action drafts?
- How should users be notified that AI monitoring is active?

## Recommended Direction

Start with advisor mode and make the output undeniably useful before adding automation. The first win is not "the bot can chat." The first win is:

> I opened Guildest and immediately understood what my community needs from me.
