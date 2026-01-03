//! Consistent Hash Partitioner - DSA Implementation
//!
//! Implements consistent hashing for Kafka partition assignment.
//! Ensures messages with the same key always go to the same partition.
//!
//! ## Time Complexity
//! - Get partition: O(log n) where n = num_partitions * virtual_nodes
//! - Build ring: O(n log n) for initial setup

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use siphasher::sip::SipHasher24;

/// Consistent hash ring for partition assignment
pub struct ConsistentHashPartitioner {
    /// Hash ring mapping hash values to partitions
    ring: BTreeMap<u64, usize>,
    /// Number of partitions
    num_partitions: usize,
    /// Virtual nodes per partition for better distribution
    virtual_nodes: usize,
}

impl ConsistentHashPartitioner {
    /// Default number of virtual nodes per partition
    const DEFAULT_VIRTUAL_NODES: usize = 150;
    
    /// Create a new partitioner
    pub fn new(num_partitions: usize) -> Self {
        Self::with_virtual_nodes(num_partitions, Self::DEFAULT_VIRTUAL_NODES)
    }
    
    /// Create with custom virtual node count
    pub fn with_virtual_nodes(num_partitions: usize, virtual_nodes: usize) -> Self {
        let mut ring = BTreeMap::new();
        
        // Build ring with virtual nodes for each partition
        for partition in 0..num_partitions {
            for vnode in 0..virtual_nodes {
                let key = format!("partition-{}-vnode-{}", partition, vnode);
                let hash = Self::hash_key(&key);
                ring.insert(hash, partition);
            }
        }
        
        Self {
            ring,
            num_partitions,
            virtual_nodes,
        }
    }
    
    /// Hash a key using SipHash for good distribution
    fn hash_key(key: &str) -> u64 {
        let mut hasher = SipHasher24::new();
        key.hash(&mut hasher);
        hasher.finish()
    }
    
    /// Get the partition for a given key
    ///
    /// Uses binary search (O(log n)) via BTreeMap to find the
    /// first hash value >= key's hash, implementing consistent hashing.
    pub fn get_partition(&self, key: &str) -> usize {
        if self.ring.is_empty() {
            return 0;
        }
        
        let hash = Self::hash_key(key);
        
        // Find the first entry with hash >= key's hash
        // BTreeMap::range is O(log n)
        match self.ring.range(hash..).next() {
            Some((_, &partition)) => partition,
            // Wrap around to first entry if we're past the last
            None => *self.ring.values().next().unwrap(),
        }
    }
    
    /// Get partition distribution statistics
    pub fn get_stats(&self) -> PartitionerStats {
        let mut distribution = vec![0usize; self.num_partitions];
        
        for &partition in self.ring.values() {
            distribution[partition] += 1;
        }
        
        let total = distribution.iter().sum::<usize>() as f64;
        let expected = total / self.num_partitions as f64;
        
        let variance = distribution.iter()
            .map(|&count| {
                let diff = count as f64 - expected;
                diff * diff
            })
            .sum::<f64>() / self.num_partitions as f64;
        
        PartitionerStats {
            num_partitions: self.num_partitions,
            virtual_nodes: self.virtual_nodes,
            total_ring_entries: self.ring.len(),
            distribution,
            variance,
            std_dev: variance.sqrt(),
        }
    }
}

/// Statistics about partition distribution
#[derive(Debug)]
pub struct PartitionerStats {
    pub num_partitions: usize,
    pub virtual_nodes: usize,
    pub total_ring_entries: usize,
    pub distribution: Vec<usize>,
    pub variance: f64,
    pub std_dev: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    #[test]
    fn test_consistent_hashing() {
        let partitioner = ConsistentHashPartitioner::new(3);
        
        // Same key should always get same partition
        let key = "test-source-123";
        let partition1 = partitioner.get_partition(key);
        let partition2 = partitioner.get_partition(key);
        assert_eq!(partition1, partition2);
        
        // Partition should be in valid range
        assert!(partition1 < 3);
    }
    
    #[test]
    fn test_distribution() {
        let partitioner = ConsistentHashPartitioner::new(6);
        let mut counts: HashMap<usize, usize> = HashMap::new();
        
        // Hash 1000 random keys
        for i in 0..1000 {
            let key = format!("key-{}", i);
            let partition = partitioner.get_partition(&key);
            *counts.entry(partition).or_insert(0) += 1;
        }
        
        // Each partition should have some keys (rough balance)
        for partition in 0..6 {
            let count = counts.get(&partition).copied().unwrap_or(0);
            // Should have at least 100 keys per partition (with some variance)
            assert!(count > 50, "Partition {} has only {} keys", partition, count);
        }
    }
}
