//! Circuit Breaker Pattern - DSA/Design Pattern Implementation
//!
//! Prevents cascading failures by stopping requests to failing services.
//! Uses exponential backoff with jitter for recovery attempts.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use tracing::{info, warn};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests pass through
    Closed,
    /// Failure threshold exceeded - requests blocked
    Open,
    /// Testing if service recovered - limited requests allowed
    HalfOpen,
}

/// Circuit breaker configuration
#[derive(Debug, Clone)]
pub struct CircuitConfig {
    /// Number of failures before opening circuit
    pub failure_threshold: u32,
    /// Base recovery timeout in seconds
    pub recovery_timeout_secs: u64,
    /// Calls allowed in half-open state
    pub half_open_max_calls: u32,
    /// Maximum backoff time in seconds
    pub max_backoff_secs: u64,
    /// Use exponential backoff
    pub exponential_backoff: bool,
}

impl Default for CircuitConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout_secs: 30,
            half_open_max_calls: 3,
            max_backoff_secs: 300,
            exponential_backoff: true,
        }
    }
}

/// Thread-safe circuit breaker
pub struct CircuitBreaker {
    config: CircuitConfig,
    state: RwLock<CircuitState>,
    failures: AtomicU32,
    successes: AtomicU32,
    half_open_calls: AtomicU32,
    retry_count: AtomicU32,
    last_failure_time: RwLock<Option<Instant>>,
    next_retry_time: RwLock<Option<Instant>>,
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitConfig) -> Self {
        Self {
            config,
            state: RwLock::new(CircuitState::Closed),
            failures: AtomicU32::new(0),
            successes: AtomicU32::new(0),
            half_open_calls: AtomicU32::new(0),
            retry_count: AtomicU32::new(0),
            last_failure_time: RwLock::new(None),
            next_retry_time: RwLock::new(None),
        }
    }
    
    /// Check if request should be allowed
    pub fn allow_request(&self) -> bool {
        let state = *self.state.read().unwrap();
        
        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let next_retry = self.next_retry_time.read().unwrap();
                if let Some(time) = *next_retry {
                    if Instant::now() >= time {
                        self.transition_to(CircuitState::HalfOpen);
                        return true;
                    }
                }
                false
            }
            CircuitState::HalfOpen => {
                self.half_open_calls.load(Ordering::SeqCst) < self.config.half_open_max_calls
            }
        }
    }
    
    /// Record a successful call
    pub fn record_success(&self) {
        self.successes.fetch_add(1, Ordering::SeqCst);
        
        let state = *self.state.read().unwrap();
        if state == CircuitState::HalfOpen {
            let calls = self.half_open_calls.fetch_add(1, Ordering::SeqCst) + 1;
            if calls >= self.config.half_open_max_calls {
                self.transition_to(CircuitState::Closed);
            }
        }
    }
    
    /// Record a failed call
    pub fn record_failure(&self) {
        let failures = self.failures.fetch_add(1, Ordering::SeqCst) + 1;
        *self.last_failure_time.write().unwrap() = Some(Instant::now());
        
        let state = *self.state.read().unwrap();
        
        if state == CircuitState::HalfOpen {
            // Single failure in half-open triggers open
            self.transition_to(CircuitState::Open);
        } else if state == CircuitState::Closed && failures >= self.config.failure_threshold {
            self.transition_to(CircuitState::Open);
        }
    }
    
    /// Transition to a new state
    fn transition_to(&self, new_state: CircuitState) {
        let mut state = self.state.write().unwrap();
        let old_state = *state;
        *state = new_state;
        
        match new_state {
            CircuitState::Open => {
                let backoff = self.calculate_backoff();
                *self.next_retry_time.write().unwrap() = Some(Instant::now() + backoff);
                self.retry_count.fetch_add(1, Ordering::SeqCst);
                warn!(
                    "Circuit OPENED after {} failures. Retry in {:?}",
                    self.failures.load(Ordering::SeqCst),
                    backoff
                );
            }
            CircuitState::HalfOpen => {
                self.half_open_calls.store(0, Ordering::SeqCst);
                info!("Circuit HALF-OPEN, testing recovery");
            }
            CircuitState::Closed => {
                self.retry_count.store(0, Ordering::SeqCst);
                self.failures.store(0, Ordering::SeqCst);
                info!("Circuit CLOSED, normal operation resumed");
            }
        }
    }
    
    /// Calculate backoff time with exponential increase and jitter
    fn calculate_backoff(&self) -> Duration {
        if !self.config.exponential_backoff {
            return Duration::from_secs(self.config.recovery_timeout_secs);
        }
        
        let retry_count = self.retry_count.load(Ordering::SeqCst);
        let base_delay = self.config.recovery_timeout_secs * (2_u64.pow(retry_count));
        let capped_delay = base_delay.min(self.config.max_backoff_secs);
        
        // Add jitter (50-100% of delay)
        let jitter_factor = 0.5 + (rand::random::<f64>() * 0.5);
        let final_delay = (capped_delay as f64 * jitter_factor) as u64;
        
        Duration::from_secs(final_delay)
    }
    
    /// Execute a function with circuit breaker protection
    pub async fn execute<F, T, E>(&self, f: F) -> Result<T, CircuitError<E>>
    where
        F: std::future::Future<Output = Result<T, E>>,
    {
        if !self.allow_request() {
            return Err(CircuitError::CircuitOpen);
        }
        
        match f.await {
            Ok(result) => {
                self.record_success();
                Ok(result)
            }
            Err(e) => {
                self.record_failure();
                Err(CircuitError::Inner(e))
            }
        }
    }
    
    /// Get current state
    pub fn state(&self) -> CircuitState {
        *self.state.read().unwrap()
    }
    
    /// Get statistics
    pub fn stats(&self) -> CircuitStats {
        CircuitStats {
            state: self.state(),
            failures: self.failures.load(Ordering::SeqCst),
            successes: self.successes.load(Ordering::SeqCst),
            retry_count: self.retry_count.load(Ordering::SeqCst),
        }
    }
}

/// Error type for circuit breaker
#[derive(Debug)]
pub enum CircuitError<E> {
    CircuitOpen,
    Inner(E),
}

/// Circuit breaker statistics
#[derive(Debug)]
pub struct CircuitStats {
    pub state: CircuitState,
    pub failures: u32,
    pub successes: u32,
    pub retry_count: u32,
}
