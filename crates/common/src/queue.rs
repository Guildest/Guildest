use anyhow::{Context, Result};
use async_trait::async_trait;
use redis::{
    AsyncCommands, Value,
    aio::MultiplexedConnection,
    streams::{StreamPendingReply, StreamRangeReply, StreamReadOptions, StreamReadReply},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueDelivery {
    pub id: String,
    pub stream: String,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamEntry {
    pub id: String,
    pub payload: String,
}

#[async_trait]
pub trait EventQueue: Send + Sync {
    async fn publish(&self, stream: &str, payload: &str) -> Result<()>;
    async fn create_consumer_group(&self, stream: &str, group: &str) -> Result<()>;
    async fn consume(
        &self,
        streams: &[&str],
        group: &str,
        consumer: &str,
        count: usize,
        block_ms: usize,
    ) -> Result<Vec<QueueDelivery>>;
    async fn ack(&self, stream: &str, group: &str, id: &str) -> Result<()>;
}

#[derive(Clone)]
pub struct RedisEventQueue {
    client: redis::Client,
}

impl RedisEventQueue {
    pub fn new(url: &str) -> Result<Self> {
        let client = redis::Client::open(url).context("failed to build redis client")?;
        Ok(Self { client })
    }

    async fn connection(&self) -> Result<MultiplexedConnection> {
        self.client
            .get_multiplexed_async_connection()
            .await
            .context("failed to connect to redis")
    }

    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let mut conn = self.connection().await?;
        let payload: Option<String> = conn
            .get(key)
            .await
            .with_context(|| format!("failed to read redis key {key}"))?;

        payload
            .map(|json| {
                serde_json::from_str(&json)
                    .with_context(|| format!("failed to decode redis json for key {key}"))
            })
            .transpose()
    }

    pub async fn set_json<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl_seconds: u64,
    ) -> Result<()> {
        let payload = serde_json::to_string(value)
            .with_context(|| format!("failed to encode redis json for key {key}"))?;
        let mut conn = self.connection().await?;
        conn.set_ex::<_, _, ()>(key, payload, ttl_seconds)
            .await
            .with_context(|| format!("failed to write redis key {key}"))?;
        Ok(())
    }

    pub async fn stream_len(&self, stream: &str) -> Result<i64> {
        let mut conn = self.connection().await?;
        redis::cmd("XLEN")
            .arg(stream)
            .query_async::<i64>(&mut conn)
            .await
            .with_context(|| format!("failed to read stream length for {stream}"))
    }

    pub async fn oldest_stream_entry_ms(&self, stream: &str) -> Result<Option<i64>> {
        let mut conn = self.connection().await?;
        let reply: StreamRangeReply = redis::cmd("XRANGE")
            .arg(stream)
            .arg("-")
            .arg("+")
            .arg("COUNT")
            .arg(1)
            .query_async(&mut conn)
            .await
            .with_context(|| format!("failed to read oldest stream entry for {stream}"))?;

        let Some(first) = reply.ids.first() else {
            return Ok(None);
        };

        Ok(parse_stream_id_ms(&first.id))
    }

    pub async fn recent_stream_payloads(&self, stream: &str, limit: usize) -> Result<Vec<String>> {
        Ok(self
            .recent_stream_entries(stream, limit)
            .await?
            .into_iter()
            .map(|entry| entry.payload)
            .collect())
    }

    pub async fn recent_stream_entries(
        &self,
        stream: &str,
        limit: usize,
    ) -> Result<Vec<StreamEntry>> {
        let mut conn = self.connection().await?;
        let reply: StreamRangeReply = redis::cmd("XREVRANGE")
            .arg(stream)
            .arg("+")
            .arg("-")
            .arg("COUNT")
            .arg(limit)
            .query_async(&mut conn)
            .await
            .with_context(|| format!("failed to read recent stream payloads for {stream}"))?;

        let mut entries = Vec::new();
        for entry in reply.ids {
            let payload = entry
                .map
                .get("payload")
                .map(redis_value_to_string)
                .transpose()?
                .context("stream message missing payload")?;
            entries.push(StreamEntry {
                id: entry.id,
                payload,
            });
        }

        Ok(entries)
    }

    pub async fn stream_entry_payload(&self, stream: &str, id: &str) -> Result<Option<String>> {
        let mut conn = self.connection().await?;
        let reply: StreamRangeReply = redis::cmd("XRANGE")
            .arg(stream)
            .arg(id)
            .arg(id)
            .query_async(&mut conn)
            .await
            .with_context(|| format!("failed to read stream entry payload for {stream}/{id}"))?;

        let Some(entry) = reply.ids.first() else {
            return Ok(None);
        };
        let payload = entry
            .map
            .get("payload")
            .map(redis_value_to_string)
            .transpose()?
            .context("stream message missing payload")?;
        Ok(Some(payload))
    }

    pub async fn delete_stream_entry(&self, stream: &str, id: &str) -> Result<()> {
        let mut conn = self.connection().await?;
        redis::cmd("XDEL")
            .arg(stream)
            .arg(id)
            .query_async::<i64>(&mut conn)
            .await
            .with_context(|| format!("failed to delete stream entry for {stream}/{id}"))?;
        Ok(())
    }

    pub async fn pending_count(&self, stream: &str, group: &str) -> Result<i64> {
        let mut conn = self.connection().await?;
        let reply: StreamPendingReply = redis::cmd("XPENDING")
            .arg(stream)
            .arg(group)
            .query_async(&mut conn)
            .await
            .with_context(|| format!("failed to read pending count for {stream}/{group}"))?;
        Ok(reply.count() as i64)
    }

    pub async fn incr_with_ttl(&self, key: &str, ttl_seconds: u64) -> Result<i64> {
        let mut conn = self.connection().await?;
        let count: i64 = conn
            .incr(key, 1_i64)
            .await
            .with_context(|| format!("failed to increment redis key {key}"))?;
        conn.expire::<_, ()>(key, ttl_seconds as i64)
            .await
            .with_context(|| format!("failed to expire redis key {key}"))?;
        Ok(count)
    }

    pub async fn del_key(&self, key: &str) -> Result<()> {
        let mut conn = self.connection().await?;
        conn.del::<_, ()>(key)
            .await
            .with_context(|| format!("failed to delete redis key {key}"))?;
        Ok(())
    }

    pub async fn claim_key_with_ttl(
        &self,
        key: &str,
        value: &str,
        ttl_seconds: u64,
    ) -> Result<bool> {
        let mut conn = self.connection().await?;
        let response: Option<String> = redis::cmd("SET")
            .arg(key)
            .arg(value)
            .arg("EX")
            .arg(ttl_seconds)
            .arg("NX")
            .query_async(&mut conn)
            .await
            .with_context(|| format!("failed to claim redis key {key}"))?;
        Ok(response.is_some())
    }

    pub async fn sorted_set_len(&self, key: &str) -> Result<i64> {
        let mut conn = self.connection().await?;
        redis::cmd("ZCARD")
            .arg(key)
            .query_async::<i64>(&mut conn)
            .await
            .with_context(|| format!("failed to read sorted set size for {key}"))
    }

    pub async fn earliest_sorted_set_score_ms(&self, key: &str) -> Result<Option<i64>> {
        let mut conn = self.connection().await?;
        let rows: Vec<(String, f64)> = redis::cmd("ZRANGE")
            .arg(key)
            .arg(0)
            .arg(0)
            .arg("WITHSCORES")
            .query_async(&mut conn)
            .await
            .with_context(|| format!("failed to read earliest sorted set score for {key}"))?;

        Ok(rows.first().map(|(_, score)| *score as i64))
    }

    pub async fn schedule_message(
        &self,
        key: &str,
        payload: &str,
        execute_at_ms: i64,
    ) -> Result<()> {
        let mut conn = self.connection().await?;
        redis::cmd("ZADD")
            .arg(key)
            .arg(execute_at_ms)
            .arg(payload)
            .query_async::<i64>(&mut conn)
            .await
            .with_context(|| format!("failed to schedule retry message for {key}"))?;
        Ok(())
    }

    pub async fn pop_due_scheduled_messages(
        &self,
        key: &str,
        now_ms: i64,
        limit: usize,
    ) -> Result<Vec<String>> {
        let mut conn = self.connection().await?;
        let payloads: Vec<String> = redis::cmd("ZRANGEBYSCORE")
            .arg(key)
            .arg("-inf")
            .arg(now_ms)
            .arg("LIMIT")
            .arg(0)
            .arg(limit)
            .query_async(&mut conn)
            .await
            .with_context(|| format!("failed to read scheduled retry messages for {key}"))?;

        let mut removed = Vec::new();
        for payload in payloads {
            let deleted: i64 = redis::cmd("ZREM")
                .arg(key)
                .arg(&payload)
                .query_async(&mut conn)
                .await
                .with_context(|| format!("failed to remove scheduled retry message for {key}"))?;
            if deleted > 0 {
                removed.push(payload);
            }
        }

        Ok(removed)
    }
}

#[async_trait]
impl EventQueue for RedisEventQueue {
    async fn publish(&self, stream: &str, payload: &str) -> Result<()> {
        let mut conn = self.connection().await?;
        redis::cmd("XADD")
            .arg(stream)
            .arg("*")
            .arg("payload")
            .arg(payload)
            .query_async::<String>(&mut conn)
            .await
            .context("failed to publish stream message")?;
        Ok(())
    }

    async fn create_consumer_group(&self, stream: &str, group: &str) -> Result<()> {
        let mut conn = self.connection().await?;
        let result = redis::cmd("XGROUP")
            .arg("CREATE")
            .arg(stream)
            .arg(group)
            .arg("$")
            .arg("MKSTREAM")
            .query_async::<()>(&mut conn)
            .await;

        match result {
            Ok(()) => Ok(()),
            Err(err) if err.code() == Some("BUSYGROUP") => Ok(()),
            Err(err) => Err(err).context("failed to create consumer group"),
        }
    }

    async fn consume(
        &self,
        streams: &[&str],
        group: &str,
        consumer: &str,
        count: usize,
        block_ms: usize,
    ) -> Result<Vec<QueueDelivery>> {
        let mut conn = self.connection().await?;
        let ids = vec![">"; streams.len()];
        let options = StreamReadOptions::default()
            .group(group, consumer)
            .count(count)
            .block(block_ms);

        let reply: StreamReadReply = conn
            .xread_options(streams, &ids, &options)
            .await
            .context("failed to read from stream")?;

        let mut deliveries = Vec::new();
        for stream in reply.keys {
            for entry in stream.ids {
                let payload = entry
                    .map
                    .get("payload")
                    .map(redis_value_to_string)
                    .transpose()?
                    .context("stream message missing payload")?;

                deliveries.push(QueueDelivery {
                    id: entry.id,
                    stream: stream.key.clone(),
                    payload,
                });
            }
        }

        Ok(deliveries)
    }

    async fn ack(&self, stream: &str, group: &str, id: &str) -> Result<()> {
        let mut conn = self.connection().await?;
        redis::cmd("XACK")
            .arg(stream)
            .arg(group)
            .arg(id)
            .query_async::<i64>(&mut conn)
            .await
            .context("failed to ack stream message")?;
        Ok(())
    }
}

fn redis_value_to_string(value: &Value) -> Result<String> {
    match value {
        Value::BulkString(bytes) => String::from_utf8(bytes.clone()).context("payload not utf-8"),
        Value::SimpleString(text) => Ok(text.clone()),
        other => Err(anyhow::anyhow!("unexpected redis payload type: {other:?}")),
    }
}

fn parse_stream_id_ms(id: &str) -> Option<i64> {
    id.split('-').next()?.parse::<i64>().ok()
}
