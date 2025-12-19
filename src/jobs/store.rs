//! Job store for tracking chunking job status.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::types::{ChunkJobStatus, ChunkJobStatusResponse};

/// In-memory job store for tracking chunking jobs.
pub struct JobStore {
    jobs: HashMap<Uuid, JobRecord>,
}

/// Internal record for tracking a job.
#[derive(Debug, Clone)]
pub struct JobRecord {
    pub job_id: Uuid,
    pub status: ChunkJobStatus,
    pub total_items: usize,
    pub processed_items: usize,
    pub chunks_created: usize,
    pub error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl JobRecord {
    /// Create a new job record.
    pub fn new(job_id: Uuid, total_items: usize) -> Self {
        Self {
            job_id,
            status: ChunkJobStatus::Pending,
            total_items,
            processed_items: 0,
            chunks_created: 0,
            error: None,
            started_at: None,
            completed_at: None,
            created_at: Utc::now(),
        }
    }

    /// Mark the job as started.
    pub fn start(&mut self) {
        self.status = ChunkJobStatus::Running;
        self.started_at = Some(Utc::now());
    }

    /// Update progress.
    pub fn update_progress(&mut self, processed: usize, chunks: usize) {
        self.processed_items = processed;
        self.chunks_created = chunks;
    }

    /// Mark the job as completed.
    pub fn complete(&mut self) {
        self.status = ChunkJobStatus::Completed;
        self.completed_at = Some(Utc::now());
    }

    /// Mark the job as failed.
    pub fn fail(&mut self, error: String) {
        self.status = ChunkJobStatus::Failed;
        self.error = Some(error);
        self.completed_at = Some(Utc::now());
    }

    /// Convert to response type.
    pub fn to_response(&self) -> ChunkJobStatusResponse {
        ChunkJobStatusResponse {
            job_id: self.job_id,
            status: self.status,
            total_items: self.total_items,
            processed_items: self.processed_items,
            chunks_created: self.chunks_created,
            error: self.error.clone(),
            started_at: self.started_at,
            completed_at: self.completed_at,
        }
    }
}

impl JobStore {
    /// Create a new job store.
    pub fn new() -> Self {
        Self {
            jobs: HashMap::new(),
        }
    }

    /// Create a new job and return its ID.
    pub fn create_job(&mut self, total_items: usize) -> Uuid {
        let job_id = Uuid::new_v4();
        let record = JobRecord::new(job_id, total_items);
        self.jobs.insert(job_id, record);
        job_id
    }

    /// Get a job by ID.
    pub fn get_job(&self, job_id: Uuid) -> Option<&JobRecord> {
        self.jobs.get(&job_id)
    }

    /// Get a mutable reference to a job.
    pub fn get_job_mut(&mut self, job_id: Uuid) -> Option<&mut JobRecord> {
        self.jobs.get_mut(&job_id)
    }

    /// Start a job.
    pub fn start_job(&mut self, job_id: Uuid) -> bool {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.start();
            true
        } else {
            false
        }
    }

    /// Update job progress.
    pub fn update_job_progress(&mut self, job_id: Uuid, processed: usize, chunks: usize) -> bool {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.update_progress(processed, chunks);
            true
        } else {
            false
        }
    }

    /// Complete a job.
    pub fn complete_job(&mut self, job_id: Uuid) -> bool {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.complete();
            true
        } else {
            false
        }
    }

    /// Fail a job.
    pub fn fail_job(&mut self, job_id: Uuid, error: String) -> bool {
        if let Some(job) = self.jobs.get_mut(&job_id) {
            job.fail(error);
            true
        } else {
            false
        }
    }

    /// Get job status as response.
    pub fn get_job_status(&self, job_id: Uuid) -> Option<ChunkJobStatusResponse> {
        self.jobs.get(&job_id).map(|j| j.to_response())
    }

    /// Clean up old completed jobs (older than 1 hour).
    pub fn cleanup_old_jobs(&mut self) {
        let cutoff = Utc::now() - chrono::Duration::hours(1);
        self.jobs.retain(|_, job| {
            match job.status {
                ChunkJobStatus::Completed | ChunkJobStatus::Failed => {
                    job.completed_at.map_or(true, |t| t > cutoff)
                }
                _ => true,
            }
        });
    }

    /// Get count of jobs by status.
    pub fn get_job_counts(&self) -> HashMap<ChunkJobStatus, usize> {
        let mut counts = HashMap::new();
        for job in self.jobs.values() {
            *counts.entry(job.status).or_insert(0) += 1;
        }
        counts
    }
}

impl Default for JobStore {
    fn default() -> Self {
        Self::new()
    }
}
