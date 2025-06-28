//! Request coalescing for performance optimization
//!
//! This module implements request coalescing to batch similar requests together,
//! reducing load on the Loxone Miniserver and improving response times.

use crate::error::{LoxoneError, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::{oneshot, RwLock};
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Configuration for request coalescing
#[derive(Debug, Clone)]
pub struct CoalescingConfig {
    /// Maximum time to wait before executing a batch
    pub max_wait_time: Duration,
    /// Maximum number of requests to batch together
    pub max_batch_size: usize,
    /// Enable coalescing for device state requests
    pub enable_device_state_coalescing: bool,
    /// Enable coalescing for room device queries
    pub enable_room_device_coalescing: bool,
    /// Enable coalescing for sensor readings
    pub enable_sensor_coalescing: bool,
}

impl Default for CoalescingConfig {
    fn default() -> Self {
        Self {
            max_wait_time: Duration::from_millis(50), // 50ms max wait
            max_batch_size: 10,
            enable_device_state_coalescing: true,
            enable_room_device_coalescing: true,
            enable_sensor_coalescing: true,
        }
    }
}

/// Types of requests that can be coalesced
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RequestType {
    DeviceState,
    RoomDevices,
    SensorReading,
    StructureInfo,
}

/// A request waiting to be coalesced
#[derive(Debug)]
pub struct PendingRequest {
    pub request_id: String,
    pub request_type: RequestType,
    pub parameters: Value,
    pub response_sender: oneshot::Sender<Result<Value>>,
    pub created_at: Instant,
}

/// A batch of coalesced requests
#[derive(Debug)]
pub struct RequestBatch {
    pub request_type: RequestType,
    pub requests: Vec<PendingRequest>,
    pub created_at: Instant,
}

impl RequestBatch {
    /// Create a new request batch
    pub fn new(request_type: RequestType) -> Self {
        Self {
            request_type,
            requests: Vec::new(),
            created_at: Instant::now(),
        }
    }

    /// Add a request to the batch
    pub fn add_request(&mut self, request: PendingRequest) {
        self.requests.push(request);
    }

    /// Check if the batch should be executed
    pub fn should_execute(&self, config: &CoalescingConfig) -> bool {
        self.requests.len() >= config.max_batch_size
            || self.created_at.elapsed() >= config.max_wait_time
    }

    /// Get the unique parameters for batching
    pub fn get_batch_parameters(&self) -> Vec<Value> {
        let mut unique_params = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for request in &self.requests {
            let param_str = request.parameters.to_string();
            if seen.insert(param_str) {
                unique_params.push(request.parameters.clone());
            }
        }

        unique_params
    }
}

/// Request coalescing manager
pub struct RequestCoalescer {
    config: CoalescingConfig,
    pending_batches: Arc<RwLock<HashMap<RequestType, RequestBatch>>>,
    executor: Arc<dyn BatchExecutor + Send + Sync>,
    metrics: Arc<Mutex<CoalescingMetrics>>,
}

/// Metrics for request coalescing
#[derive(Debug, Default, Clone)]
pub struct CoalescingMetrics {
    pub requests_coalesced: u64,
    pub batches_executed: u64,
    pub average_batch_size: f64,
    pub time_saved_ms: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
}

impl CoalescingMetrics {
    /// Update metrics when a batch is executed
    pub fn batch_executed(&mut self, batch_size: usize, time_saved: Duration) {
        self.batches_executed += 1;
        self.requests_coalesced += batch_size as u64;
        self.time_saved_ms += time_saved.as_millis() as u64;

        // Update average batch size
        self.average_batch_size = self.requests_coalesced as f64 / self.batches_executed as f64;
    }

    /// Record cache hit
    pub fn cache_hit(&mut self) {
        self.cache_hits += 1;
    }

    /// Record cache miss
    pub fn cache_miss(&mut self) {
        self.cache_misses += 1;
    }
}

/// Trait for executing batched requests
#[async_trait::async_trait]
pub trait BatchExecutor {
    /// Execute a batch of device state requests
    async fn execute_device_state_batch(
        &self,
        device_uuids: Vec<String>,
    ) -> Result<HashMap<String, Value>>;

    /// Execute a batch of room device queries
    async fn execute_room_devices_batch(
        &self,
        room_uuids: Vec<String>,
    ) -> Result<HashMap<String, Vec<Value>>>;

    /// Execute a batch of sensor readings
    async fn execute_sensor_batch(
        &self,
        sensor_uuids: Vec<String>,
    ) -> Result<HashMap<String, Value>>;

    /// Execute a batch of structure info requests
    async fn execute_structure_batch(
        &self,
        info_types: Vec<String>,
    ) -> Result<HashMap<String, Value>>;
}

impl RequestCoalescer {
    /// Create a new request coalescer
    pub fn new(config: CoalescingConfig, executor: Arc<dyn BatchExecutor + Send + Sync>) -> Self {
        Self {
            config,
            pending_batches: Arc::new(RwLock::new(HashMap::new())),
            executor,
            metrics: Arc::new(Mutex::new(CoalescingMetrics::default())),
        }
    }

    /// Submit a request for potential coalescing
    pub async fn submit_request(
        self: &Arc<Self>,
        request_id: String,
        request_type: RequestType,
        parameters: Value,
    ) -> Result<Value> {
        // Check if coalescing is enabled for this request type
        if !self.is_coalescing_enabled(&request_type) {
            return self
                .execute_single_request(&request_type, &parameters)
                .await;
        }

        let (tx, rx) = oneshot::channel();
        let pending_request = PendingRequest {
            request_id,
            request_type: request_type.clone(),
            parameters,
            response_sender: tx,
            created_at: Instant::now(),
        };

        // Add to pending batch
        self.add_to_batch(pending_request).await;

        // Wait for response with timeout
        let response = timeout(Duration::from_secs(5), rx)
            .await
            .map_err(|_| LoxoneError::timeout("Request coalescing timeout"))?
            .map_err(|_| LoxoneError::config("Response channel closed"))?;

        response
    }

    /// Add a request to the appropriate batch
    async fn add_to_batch(self: &Arc<Self>, request: PendingRequest) {
        let request_type = request.request_type.clone();
        let mut batches = self.pending_batches.write().await;

        let batch = batches
            .entry(request_type.clone())
            .or_insert_with(|| RequestBatch::new(request_type.clone()));

        batch.add_request(request);

        // Check if batch should be executed immediately
        if batch.should_execute(&self.config) {
            let batch_to_execute = batches.remove(&request_type).unwrap();
            drop(batches); // Release the lock before async execution

            tokio::spawn(self.clone().execute_batch(batch_to_execute));
        }
    }

    /// Execute a batch of requests
    async fn execute_batch(self: Arc<Self>, batch: RequestBatch) {
        let start_time = Instant::now();
        let batch_size = batch.requests.len();

        debug!(
            "Executing batch of {} {} requests",
            batch_size,
            format!("{:?}", batch.request_type)
        );

        let result = match batch.request_type {
            RequestType::DeviceState => {
                let device_uuids: Vec<String> = batch
                    .requests
                    .iter()
                    .filter_map(|r| {
                        r.parameters
                            .get("uuid")
                            .and_then(|v| v.as_str().map(String::from))
                    })
                    .collect();

                self.executor
                    .execute_device_state_batch(device_uuids)
                    .await
                    .map(|results| Value::Object(results.into_iter().collect()))
            }

            RequestType::RoomDevices => {
                let room_uuids: Vec<String> = batch
                    .requests
                    .iter()
                    .filter_map(|r| {
                        r.parameters
                            .get("room_uuid")
                            .and_then(|v| v.as_str().map(String::from))
                    })
                    .collect();

                self.executor
                    .execute_room_devices_batch(room_uuids)
                    .await
                    .map(|results| {
                        Value::Object(
                            results
                                .into_iter()
                                .map(|(k, v)| (k, Value::Array(v)))
                                .collect(),
                        )
                    })
            }

            RequestType::SensorReading => {
                let sensor_uuids: Vec<String> = batch
                    .requests
                    .iter()
                    .filter_map(|r| {
                        r.parameters
                            .get("sensor_uuid")
                            .and_then(|v| v.as_str().map(String::from))
                    })
                    .collect();

                self.executor
                    .execute_sensor_batch(sensor_uuids)
                    .await
                    .map(|results| Value::Object(results.into_iter().collect()))
            }

            RequestType::StructureInfo => {
                let info_types: Vec<String> = batch
                    .requests
                    .iter()
                    .filter_map(|r| {
                        r.parameters
                            .get("info_type")
                            .and_then(|v| v.as_str().map(String::from))
                    })
                    .collect();

                self.executor
                    .execute_structure_batch(info_types)
                    .await
                    .map(|results| Value::Object(results.into_iter().collect()))
            }
        };

        let execution_time = start_time.elapsed();

        // Update metrics
        {
            let mut metrics = self.metrics.lock().unwrap();
            metrics.batch_executed(batch_size, execution_time);
        }

        // Send responses to individual requests
        match result {
            Ok(batch_results) => {
                for request in batch.requests {
                    let individual_result =
                        self.extract_individual_result(&request, &batch_results);
                    let _ = request.response_sender.send(individual_result);
                }

                info!(
                    "Successfully executed batch of {} requests in {:?}",
                    batch_size, execution_time
                );
            }
            Err(error) => {
                warn!("Batch execution failed: {}", error);
                let error_msg = error.to_string();
                for request in batch.requests {
                    let _ = request
                        .response_sender
                        .send(Err(LoxoneError::config(error_msg.clone())));
                }
            }
        }
    }

    /// Extract individual result from batch results
    fn extract_individual_result(
        &self,
        request: &PendingRequest,
        batch_results: &Value,
    ) -> Result<Value> {
        match request.request_type {
            RequestType::DeviceState => {
                if let Some(uuid) = request.parameters.get("uuid").and_then(|v| v.as_str()) {
                    if let Some(result) = batch_results.get(uuid) {
                        Ok(result.clone())
                    } else {
                        Ok(serde_json::json!({
                            "uuid": uuid,
                            "state": null,
                            "message": "Device not found in batch results"
                        }))
                    }
                } else {
                    Err(LoxoneError::config("Missing device UUID in request"))
                }
            }

            RequestType::RoomDevices => {
                if let Some(room_uuid) =
                    request.parameters.get("room_uuid").and_then(|v| v.as_str())
                {
                    if let Some(result) = batch_results.get(room_uuid) {
                        Ok(result.clone())
                    } else {
                        Ok(serde_json::json!({
                            "room_uuid": room_uuid,
                            "devices": [],
                            "message": "Room not found in batch results"
                        }))
                    }
                } else {
                    Err(LoxoneError::config("Missing room UUID in request"))
                }
            }

            RequestType::SensorReading => {
                if let Some(sensor_uuid) = request
                    .parameters
                    .get("sensor_uuid")
                    .and_then(|v| v.as_str())
                {
                    if let Some(result) = batch_results.get(sensor_uuid) {
                        Ok(result.clone())
                    } else {
                        Ok(serde_json::json!({
                            "sensor_uuid": sensor_uuid,
                            "value": null,
                            "message": "Sensor not found in batch results"
                        }))
                    }
                } else {
                    Err(LoxoneError::config("Missing sensor UUID in request"))
                }
            }

            RequestType::StructureInfo => {
                if let Some(info_type) =
                    request.parameters.get("info_type").and_then(|v| v.as_str())
                {
                    if let Some(result) = batch_results.get(info_type) {
                        Ok(result.clone())
                    } else {
                        Ok(serde_json::json!({
                            "info_type": info_type,
                            "data": null,
                            "message": "Info type not found in batch results"
                        }))
                    }
                } else {
                    Err(LoxoneError::config("Missing info type in request"))
                }
            }
        }
    }

    /// Execute a single request without coalescing
    async fn execute_single_request(
        &self,
        request_type: &RequestType,
        parameters: &Value,
    ) -> Result<Value> {
        match request_type {
            RequestType::DeviceState => {
                if let Some(uuid) = parameters.get("uuid").and_then(|v| v.as_str()) {
                    let result = self
                        .executor
                        .execute_device_state_batch(vec![uuid.to_string()])
                        .await?;
                    Ok(result.get(uuid).cloned().unwrap_or(Value::Null))
                } else {
                    Err(LoxoneError::config("Missing device UUID"))
                }
            }

            RequestType::RoomDevices => {
                if let Some(room_uuid) = parameters.get("room_uuid").and_then(|v| v.as_str()) {
                    let result = self
                        .executor
                        .execute_room_devices_batch(vec![room_uuid.to_string()])
                        .await?;
                    Ok(Value::Array(
                        result.get(room_uuid).cloned().unwrap_or_default(),
                    ))
                } else {
                    Err(LoxoneError::config("Missing room UUID"))
                }
            }

            RequestType::SensorReading => {
                if let Some(sensor_uuid) = parameters.get("sensor_uuid").and_then(|v| v.as_str()) {
                    let result = self
                        .executor
                        .execute_sensor_batch(vec![sensor_uuid.to_string()])
                        .await?;
                    Ok(result.get(sensor_uuid).cloned().unwrap_or(Value::Null))
                } else {
                    Err(LoxoneError::config("Missing sensor UUID"))
                }
            }

            RequestType::StructureInfo => {
                if let Some(info_type) = parameters.get("info_type").and_then(|v| v.as_str()) {
                    let result = self
                        .executor
                        .execute_structure_batch(vec![info_type.to_string()])
                        .await?;
                    Ok(result.get(info_type).cloned().unwrap_or(Value::Null))
                } else {
                    Err(LoxoneError::config("Missing info type"))
                }
            }
        }
    }

    /// Check if coalescing is enabled for a request type
    fn is_coalescing_enabled(&self, request_type: &RequestType) -> bool {
        match request_type {
            RequestType::DeviceState => self.config.enable_device_state_coalescing,
            RequestType::RoomDevices => self.config.enable_room_device_coalescing,
            RequestType::SensorReading => self.config.enable_sensor_coalescing,
            RequestType::StructureInfo => true, // Always enable for structure info
        }
    }

    /// Get current metrics
    pub fn get_metrics(&self) -> CoalescingMetrics {
        self.metrics.lock().unwrap().clone()
    }

    /// Start the batch processor background task
    pub fn start_batch_processor(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(10));

            loop {
                interval.tick().await;

                // Check for batches that should be executed due to timeout
                let batches_to_execute = {
                    let mut pending = self.pending_batches.write().await;
                    let mut to_execute = Vec::new();

                    pending.retain(|request_type, batch| {
                        if batch.should_execute(&self.config) {
                            to_execute.push((
                                request_type.clone(),
                                std::mem::replace(batch, RequestBatch::new(request_type.clone())),
                            ));
                            false
                        } else {
                            true
                        }
                    });

                    to_execute
                };

                // Execute timed-out batches
                for (_, batch) in batches_to_execute {
                    if !batch.requests.is_empty() {
                        tokio::spawn(self.clone().execute_batch(batch));
                    }
                }
            }
        })
    }
}

impl Clone for RequestCoalescer {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            pending_batches: self.pending_batches.clone(),
            executor: self.executor.clone(),
            metrics: self.metrics.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct MockBatchExecutor {
        call_count: AtomicUsize,
    }

    impl MockBatchExecutor {
        fn new() -> Self {
            Self {
                call_count: AtomicUsize::new(0),
            }
        }

        fn get_call_count(&self) -> usize {
            self.call_count.load(Ordering::Relaxed)
        }
    }

    #[async_trait::async_trait]
    impl BatchExecutor for MockBatchExecutor {
        async fn execute_device_state_batch(
            &self,
            device_uuids: Vec<String>,
        ) -> Result<HashMap<String, Value>> {
            self.call_count.fetch_add(1, Ordering::Relaxed);

            let mut results = HashMap::new();
            for uuid in device_uuids {
                results.insert(
                    uuid.clone(),
                    serde_json::json!({
                        "uuid": uuid,
                        "state": "on",
                        "value": 1.0
                    }),
                );
            }
            Ok(results)
        }

        async fn execute_room_devices_batch(
            &self,
            room_uuids: Vec<String>,
        ) -> Result<HashMap<String, Vec<Value>>> {
            self.call_count.fetch_add(1, Ordering::Relaxed);

            let mut results = HashMap::new();
            for uuid in room_uuids {
                results.insert(uuid.clone(), vec![
                    serde_json::json!({"uuid": format!("{}-device1", uuid), "name": "Device 1"}),
                    serde_json::json!({"uuid": format!("{}-device2", uuid), "name": "Device 2"}),
                ]);
            }
            Ok(results)
        }

        async fn execute_sensor_batch(
            &self,
            sensor_uuids: Vec<String>,
        ) -> Result<HashMap<String, Value>> {
            self.call_count.fetch_add(1, Ordering::Relaxed);

            let mut results = HashMap::new();
            for uuid in sensor_uuids {
                results.insert(
                    uuid.clone(),
                    serde_json::json!({
                        "uuid": uuid,
                        "value": 23.5,
                        "unit": "°C"
                    }),
                );
            }
            Ok(results)
        }

        async fn execute_structure_batch(
            &self,
            info_types: Vec<String>,
        ) -> Result<HashMap<String, Value>> {
            self.call_count.fetch_add(1, Ordering::Relaxed);

            let mut results = HashMap::new();
            for info_type in info_types {
                results.insert(
                    info_type.clone(),
                    serde_json::json!({
                        "type": info_type,
                        "data": {"mock": "data"}
                    }),
                );
            }
            Ok(results)
        }
    }

    #[tokio::test]
    async fn test_request_coalescing() {
        let executor = Arc::new(MockBatchExecutor::new());
        let config = CoalescingConfig {
            max_wait_time: Duration::from_millis(100),
            max_batch_size: 3,
            ..Default::default()
        };

        let coalescer = Arc::new(RequestCoalescer::new(config, executor.clone()));

        // Start batch processor
        let _processor_handle = coalescer.clone().start_batch_processor();

        // Submit multiple device state requests
        let mut handles = Vec::new();
        for i in 1..=5 {
            let coalescer_clone = coalescer.clone();
            let handle = tokio::spawn(async move {
                coalescer_clone
                    .submit_request(
                        format!("req-{i}"),
                        RequestType::DeviceState,
                        serde_json::json!({"uuid": format!("device-{i}")}),
                    )
                    .await
            });
            handles.push(handle);
        }

        // Wait for all requests to complete
        for handle in handles {
            let result = handle.await.unwrap().unwrap();
            assert!(result.get("uuid").is_some());
            assert_eq!(result.get("state").unwrap(), "on");
        }

        // Should have made 2 batch calls (3 + 2 requests)
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(executor.get_call_count(), 2);

        // Check metrics
        let metrics = coalescer.get_metrics();
        assert_eq!(metrics.batches_executed, 2);
        assert_eq!(metrics.requests_coalesced, 5);
    }

    #[tokio::test]
    async fn test_batch_timeout() {
        let executor = Arc::new(MockBatchExecutor::new());
        let config = CoalescingConfig {
            max_wait_time: Duration::from_millis(50),
            max_batch_size: 10, // Large batch size to test timeout
            ..Default::default()
        };

        let coalescer = Arc::new(RequestCoalescer::new(config, executor.clone()));
        let _processor_handle = coalescer.clone().start_batch_processor();

        // Submit a single request
        let result = coalescer
            .submit_request(
                "timeout-test".to_string(),
                RequestType::DeviceState,
                serde_json::json!({"uuid": "test-device"}),
            )
            .await
            .unwrap();

        assert_eq!(result.get("uuid").unwrap(), "test-device");

        // Should have made 1 call due to timeout
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(executor.get_call_count(), 1);
    }

    #[test]
    fn test_coalescing_config() {
        let config = CoalescingConfig::default();
        assert_eq!(config.max_wait_time, Duration::from_millis(50));
        assert_eq!(config.max_batch_size, 10);
        assert!(config.enable_device_state_coalescing);
        assert!(config.enable_room_device_coalescing);
        assert!(config.enable_sensor_coalescing);
    }

    #[test]
    fn test_request_batch() {
        let mut batch = RequestBatch::new(RequestType::DeviceState);
        assert_eq!(batch.requests.len(), 0);

        let config = CoalescingConfig::default();
        assert!(!batch.should_execute(&config));

        // Add requests to trigger batch size limit
        for i in 0..10 {
            let (tx, _) = oneshot::channel();
            batch.add_request(PendingRequest {
                request_id: format!("req-{i}"),
                request_type: RequestType::DeviceState,
                parameters: serde_json::json!({"uuid": format!("device-{i}")}),
                response_sender: tx,
                created_at: Instant::now(),
            });
        }

        assert!(batch.should_execute(&config));
    }
}
