# Moderation Worker
- Reads queue messages.
- Sends metadata to Groq Llama Guard 4 for toxicity checks.
- Writes moderation logs to Postgres; alerts guild mod-log channels.
- Avoids storing full message content unless necessary for audit.
