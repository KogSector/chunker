//! Kafka Consumer for Chunker Service
//!
//! Consumes `code.normalized` events from Kafka and processes them
//! through the chunking pipeline.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rdkafka::config::ClientConfig;
use rdkafka::consumer::{Consumer, StreamConsumer, CommitMode};
use rdkafka::message::Message;
use rdkafka::error::KafkaError;
use tokio::sync::mpsc;
use tracing::{info, error, warn, instrument};
use serde::{Deserialize, Serialize};

/// Event received when code is normalized
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeNormalizedEvent {
    pub event_id: String,
    pub source_id: String,
    pub file_path: String,
    pub language: String,
    pub normalized_content: String,
    pub entities: Vec<CodeEntity>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeEntity {
    pub entity_type: String,  // "function", "class", "interface", etc.
    pub name: String,
    pub start_line: u32,
    pub end_line: u32,
    pub content: String,
}

/// Configuration for the Kafka consumer
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    pub bootstrap_servers: String,
    pub group_id: String,
    pub topics: Vec<String>,
    pub auto_offset_reset: String,
    pub max_poll_interval_ms: u32,
    pub session_timeout_ms: u32,
}

impl Default for ConsumerConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            group_id: "chunker".to_string(),
            topics: vec!["code.normalized".to_string()],
            auto_offset_reset: "earliest".to_string(),
            max_poll_interval_ms: 300000,  // 5 minutes for long processing
            session_timeout_ms: 30000,
        }
    }
}

/// Kafka consumer for the chunker service
pub struct KafkaChunkConsumer {
    consumer: Arc<StreamConsumer>,
    config: ConsumerConfig,
}

impl KafkaChunkConsumer {
    /// Create a new Kafka consumer
    pub fn new(config: ConsumerConfig) -> Result<Self, KafkaError> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &config.bootstrap_servers)
            .set("group.id", &config.group_id)
            .set("auto.offset.reset", &config.auto_offset_reset)
            .set("enable.auto.commit", "false")
            .set("max.poll.interval.ms", config.max_poll_interval_ms.to_string())
            .set("session.timeout.ms", config.session_timeout_ms.to_string())
            .create()?;
        
        info!(
            bootstrap = %config.bootstrap_servers,
            group = %config.group_id,
            "Kafka consumer created"
        );
        
        Ok(Self {
            consumer: Arc::new(consumer),
            config,
        })
    }
    
    /// Subscribe to configured topics
    pub fn subscribe(&self) -> Result<(), KafkaError> {
        let topics: Vec<&str> = self.config.topics.iter().map(|s| s.as_str()).collect();
        self.consumer.subscribe(&topics)?;
        info!(topics = ?self.config.topics, "Subscribed to topics");
        Ok(())
    }
    
    /// Consume messages and send them to a channel for processing
    #[instrument(skip(self, sender))]
    pub async fn consume_to_channel(
        &self,
        sender: mpsc::Sender<CodeNormalizedEvent>,
    ) -> Result<(), KafkaError> {
        use rdkafka::message::BorrowedMessage;
        use tokio_stream::StreamExt;
        
        info!("Starting Kafka consumer loop");
        
        let stream = self.consumer.stream();
        tokio::pin!(stream);
        
        while let Some(result) = stream.next().await {
            match result {
                Ok(message) => {
                    if let Some(payload) = message.payload() {
                        match serde_json::from_slice::<CodeNormalizedEvent>(payload) {
                            Ok(event) => {
                                if sender.send(event.clone()).await.is_err() {
                                    warn!("Channel closed, stopping consumer");
                                    break;
                                }
                                
                                // Manual commit after successful processing
                                if let Err(e) = self.consumer.commit_message(&message, CommitMode::Async) {
                                    error!(error = %e, "Failed to commit offset");
                                }
                            }
                            Err(e) => {
                                error!(
                                    error = %e,
                                    topic = %message.topic(),
                                    partition = %message.partition(),
                                    "Failed to deserialize message"
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    error!(error = %e, "Kafka consumer error");
                }
            }
        }
        
        Ok(())
    }
    
    /// Consume a batch of messages
    pub async fn consume_batch(
        &self,
        batch_size: usize,
        timeout: Duration,
    ) -> Vec<CodeNormalizedEvent> {
        use rdkafka::util::Timeout;
        
        let mut events = Vec::with_capacity(batch_size);
        let deadline = tokio::time::Instant::now() + timeout;
        
        while events.len() < batch_size && tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(
                deadline.saturating_duration_since(tokio::time::Instant::now()),
                async {
                    // Poll with short timeout
                    self.consumer.recv().await
                }
            ).await {
                Ok(Ok(message)) => {
                    if let Some(payload) = message.payload() {
                        if let Ok(event) = serde_json::from_slice::<CodeNormalizedEvent>(payload) {
                            events.push(event);
                            let _ = self.consumer.commit_message(&message, CommitMode::Async);
                        }
                    }
                }
                Ok(Err(e)) => {
                    error!(error = %e, "Error receiving message");
                    break;
                }
                Err(_) => {
                    // Timeout reached
                    break;
                }
            }
        }
        
        events
    }
}
