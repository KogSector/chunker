//! RabbitMQ Client for Chunker Service
//!
//! Provides async RabbitMQ operations for task coordination
//! and worker communication.

use std::sync::Arc;
use std::time::Duration;

use lapin::{
    Connection, ConnectionProperties, Channel,
    options::*, types::FieldTable,
    BasicProperties,
};
use deadpool_lapin::{Config, Manager, Pool, Runtime};
use tokio::sync::RwLock;
use tracing::{info, error, instrument};
use serde::{Deserialize, Serialize};

/// RabbitMQ connection configuration
#[derive(Debug, Clone)]
pub struct RabbitConfig {
    pub uri: String,
    pub pool_size: usize,
}

impl Default for RabbitConfig {
    fn default() -> Self {
        Self {
            uri: "amqp://confuse:confuse_dev_pass@localhost:5672".to_string(),
            pool_size: 10,
        }
    }
}

/// RabbitMQ client with connection pooling
pub struct RabbitClient {
    pool: Pool,
    config: RabbitConfig,
}

impl RabbitClient {
    /// Create a new RabbitMQ client
    pub async fn new(config: RabbitConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let cfg = Config {
            url: Some(config.uri.clone()),
            ..Default::default()
        };
        
        let pool = cfg.create_pool(Some(Runtime::Tokio1))?;
        
        info!(uri = %config.uri, "RabbitMQ client created");
        
        Ok(Self { pool, config })
    }
    
    /// Get a channel from the pool
    async fn get_channel(&self) -> Result<Channel, Box<dyn std::error::Error>> {
        let conn = self.pool.get().await?;
        let channel = conn.create_channel().await?;
        Ok(channel)
    }
    
    /// Publish a message to an exchange
    #[instrument(skip(self, payload))]
    pub async fn publish(
        &self,
        exchange: &str,
        routing_key: &str,
        payload: &[u8],
        priority: Option<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let channel = self.get_channel().await?;
        
        let properties = BasicProperties::default()
            .with_delivery_mode(2) // Persistent
            .with_priority(priority.unwrap_or(5))
            .with_content_type("application/json".into());
        
        channel.basic_publish(
            exchange,
            routing_key,
            BasicPublishOptions::default(),
            payload,
            properties,
        ).await?;
        
        info!(exchange = %exchange, routing_key = %routing_key, "Message published");
        
        Ok(())
    }
    
    /// Consume messages from a queue
    pub async fn consume<F, Fut>(
        &self,
        queue: &str,
        handler: F,
    ) -> Result<(), Box<dyn std::error::Error>>
    where
        F: Fn(Vec<u8>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = bool> + Send,
    {
        let channel = self.get_channel().await?;
        
        channel.basic_qos(10, BasicQosOptions::default()).await?;
        
        let mut consumer = channel.basic_consume(
            queue,
            "chunker-consumer",
            BasicConsumeOptions::default(),
            FieldTable::default(),
        ).await?;
        
        info!(queue = %queue, "Started consuming");
        
        use futures::StreamExt;
        
        while let Some(delivery) = consumer.next().await {
            match delivery {
                Ok(delivery) => {
                    let data = delivery.data.clone();
                    let success = handler(data).await;
                    
                    if success {
                        delivery.ack(BasicAckOptions::default()).await?;
                    } else {
                        // Requeue on failure
                        delivery.nack(BasicNackOptions { requeue: true, ..Default::default() }).await?;
                    }
                }
                Err(e) => {
                    error!(error = %e, "Consumer error");
                }
            }
        }
        
        Ok(())
    }
}

/// Notification event for service communication
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationEvent {
    pub event_type: String,
    pub source_id: String,
    pub message: String,
    pub metadata: std::collections::HashMap<String, String>,
}
