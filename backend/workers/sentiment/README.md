# Sentiment / ML Worker

Generates periodic sentiment summaries from aggregated data.

## Current mode
- Consume queue events to observe sentiment inputs.
- Write neutral placeholder sentiment rows when `DATABASE_URL` is set.

## Target behavior
- Run daily/weekly summarizations over analytics aggregates.
- Use Llama 3.1 / Groq for summarization.
- Do not track per-user sentiment to stay within Discord ToS.
