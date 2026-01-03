//! Kafka Producer for Chunker Service
//!
//! Publishes `chunk.created` events to Kafka for downstream embedding generation.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord, DeliveryFuture};
use rdkafka::error::KafkaError;
use tracing::{info, error, instrument};
use serde::{Deserialize, Serialize};

use super::consistent_hash::ConsistentHashPartitioner;

/// Event published when a chunk is created
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkCreatedEvent {
    pub event_id: String,
    pub source_id: String,
    pub file_path: String,
    pub chunk_id: String,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub content: String,
    pub token_count: u32,
    pub metadata: ChunkMetadata,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMetadata {
    pub language: Option<String>,
    pub entity_type: Option<String>,
    pub entity_name: Option<String>,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub profile: String,
}

/// Configuration for the Kafka producer
#[derive(Debug, Clone)]
pub struct ProducerConfig {
    pub bootstrap_servers: String,
    pub client_id: String,
    pub acks: String,
    pub retries: u32,
    pub compression_type: String,
    pub batch_size: u32,
    pub linger_ms: u32,
    pub num_partitions: u32,
}

impl Default for ProducerConfig {
    fn default() -> Self {
        Self {
            bootstrap_servers: "localhost:9092".to_string(),
            client_id: "chunker".to_string(),
            acks: "all".to_string(),
            retries: 3,
            compression_type: "snappy".to_string(),
            batch_size: 16384,
            linger_ms: 10,
            num_partitions: 6,
        }
    }
}

/// Kafka producer for publishing chunk events
pub struct KafkaChunkProducer {
    producer: Arc<FutureProducer>,
    config: ProducerConfig,
    partitioner: ConsistentHashPartitioner,
}

impl KafkaChunkProducer {
    /// Topic for chunk.created events
    pub const TOPIC_CHUNK_CREATED: &'static str = "chunk.created";
    
    /// Create a new Kafka producer
    pub fn new(config: ProducerConfig) -> Result<Self, KafkaError> {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &config.bootstrap_servers)
            .set("client.id", &config.client_id)
            .set("acks", &config.acks)
            .set("retries", config.retries.to_string())
            .set("compression.type", &config.compression_type)
            .set("batch.size", config.batch_size.to_string())
            .set("linger.ms", config.linger_ms.to_string())
            .set("enable.idempotence", "true")
            .create()?;
        
        let partitioner = ConsistentHashPartitioner::new(config.num_partitions as usize);
        
        info!(
            bootstrap = %config.bootstrap_servers,
            client_id = %config.client_id,
            "Kafka producer created"
        );
        
        Ok(Self {
            producer: Arc::new(producer),
            config,
            partitioner,
        })
    }
    
    /// Publish a chunk created event
    #[instrument(skip(self, event), fields(chunk_id = %event.chunk_id))]
    pub async fn publish_chunk_created(
        &self,
        event: ChunkCreatedEvent,
    ) -> Result<(), KafkaError> {
        let key = event.chunk_id.clone();
        let partition = self.partitioner.get_partition(&key);
        let payload = serde_json::to_string(&event)
            .map_err(|e| KafkaError::MessageProduction(
                rdkafka::types::RDKafkaErrorCode::InvalidArg
            ))?;
        
        let record = FutureRecord::to(Self::TOPIC_CHUNK_CREATED)
            .key(&key)
            .payload(&payload)
            .partition(partition as i32);
        
        let delivery_status = self.producer
            .send(record, Duration::from_secs(10))
            .await;
        
        match delivery_status {
            Ok((partition, offset)) => {
                info!(
                    chunk_id = %event.chunk_id,
                    partition = partition,
                    offset = offset,
                    "Chunk published successfully"
                );
                Ok(())
            }
            Err((e, _)) => {
                error!(
                    chunk_id = %event.chunk_id,
                    error = %e,
                    "Failed to publish chunk"
                );
                Err(e)
            }
        }
    }
    
    /// Publish multiple chunks in batch
    pub async fn publish_chunks_batch(
        &self,
        events: Vec<ChunkCreatedEvent>,
    ) -> Vec<Result<(), KafkaError>> {
        let futures: Vec<_> = events.into_iter()
            .map(|event| self.publish_chunk_created(event))
            .collect();
        
        futures::future::join_all(futures).await
    }
    
    /// Flush all pending messages
    pub fn flush(&self, timeout: Duration) {
        self.producer.flush(timeout);
    }
}
