//! # ConFuse Messaging Module
//! 
//! Provides Kafka and RabbitMQ integration for the Chunker service.
//! 
//! ## Features
//! - Kafka consumer for receiving code.normalized events
//! - Kafka producer for publishing chunk.created events
//! - RabbitMQ client for task queues
//! - DSA-optimized components (consistent hashing, circuit breaker)

pub mod kafka_consumer;
pub mod kafka_producer;
pub mod rabbit_client;
pub mod circuit_breaker;
pub mod consistent_hash;

pub use kafka_consumer::KafkaChunkConsumer;
pub use kafka_producer::KafkaChunkProducer;
pub use rabbit_client::RabbitClient;
pub use circuit_breaker::CircuitBreaker;
pub use consistent_hash::ConsistentHashPartitioner;
