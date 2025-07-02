//! MCP Consent Flow for Sensitive Operations
//!
//! This module implements a comprehensive consent management system for sensitive
//! operations in accordance with MCP (Model Context Protocol) best practices.
//!
//! Features:
//! - Operation sensitivity classification
//! - User consent request workflows
//! - Configurable approval requirements
//! - Bulk operation consent handling
//! - Time-based consent expiration
//! - Audit trail for consent decisions

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};
use uuid::Uuid;

/// Sensitivity levels for different operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SensitivityLevel {
    /// Low sensitivity - informational operations
    Low,
    /// Medium sensitivity - device state changes
    Medium,
    /// High sensitivity - security/safety critical operations
    High,
    /// Critical sensitivity - system-wide changes
    Critical,
}

/// Types of operations that may require consent
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationType {
    /// Device control operations
    DeviceControl {
        device_uuid: String,
        device_name: String,
        command: String,
    },
    /// Bulk device operations
    BulkDeviceControl {
        device_count: usize,
        room_name: Option<String>,
        operation_type: String,
    },
    /// Security system operations
    SecurityControl { action: String, scope: String },
    /// System configuration changes
    SystemConfiguration {
        setting: String,
        old_value: Option<String>,
        new_value: String,
    },
    /// Data export operations
    DataExport { data_type: String, scope: String },
    /// Connection management
    ConnectionManagement { action: String, target: String },
}

/// Consent request information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRequest {
    /// Unique request ID
    pub id: Uuid,

    /// Operation being requested
    pub operation: OperationType,

    /// Sensitivity level of the operation
    pub sensitivity: SensitivityLevel,

    /// Human-readable description
    pub description: String,

    /// Detailed explanation of what will happen
    pub details: String,

    /// Potential risks or consequences
    pub risks: Vec<String>,

    /// Expected duration or impact
    pub impact: String,

    /// Whether this is part of a bulk operation
    pub is_bulk: bool,

    /// Request timestamp
    pub created_at: SystemTime,

    /// Auto-approval timeout (if configured)
    pub timeout: Option<Duration>,

    /// Source of the request (user, automation, etc.)
    pub source: String,

    /// Additional context metadata
    pub metadata: HashMap<String, String>,
}

/// Consent response from user
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentResponse {
    /// Request ID being responded to
    pub request_id: Uuid,

    /// Whether consent is granted
    pub approved: bool,

    /// User-provided reason (optional)
    pub reason: Option<String>,

    /// Response timestamp
    pub responded_at: SystemTime,

    /// How long this consent is valid (for similar operations)
    pub validity_duration: Option<Duration>,

    /// Whether to apply this decision to similar operations
    pub apply_to_similar: bool,

    /// User identifier
    pub user_id: Option<String>,
}

/// Consent decision record for audit trail
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentRecord {
    /// Original request
    pub request: ConsentRequest,

    /// User response
    pub response: ConsentResponse,

    /// Final decision
    pub decision: ConsentDecision,

    /// How the decision was made
    pub decision_method: DecisionMethod,

    /// Execution result (if operation was performed)
    pub execution_result: Option<ExecutionResult>,
}

/// Final consent decision
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConsentDecision {
    /// Operation approved and should proceed
    Approved,
    /// Operation denied
    Denied { reason: String },
    /// Operation timed out waiting for response
    TimedOut,
    /// Operation was auto-approved based on policy
    AutoApproved { policy: String },
}

/// How the consent decision was made
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DecisionMethod {
    /// User explicitly approved/denied
    UserDecision,
    /// Automatic approval based on policy
    PolicyBased,
    /// Timed out waiting for user response
    Timeout,
    /// Previously granted consent still valid
    CachedConsent,
}

/// Result of executing the consented operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    /// Whether execution was successful
    pub success: bool,

    /// Error message if execution failed
    pub error: Option<String>,

    /// Execution timestamp
    pub executed_at: SystemTime,

    /// Duration of execution
    pub duration: Duration,

    /// Any relevant output or results
    pub output: Option<String>,
}

/// Configuration for consent management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentConfig {
    /// Whether consent is required at all
    pub enabled: bool,

    /// Default timeout for consent requests
    pub default_timeout: Duration,

    /// Operations that require consent by sensitivity level
    pub required_for_sensitivity: HashSet<SensitivityLevel>,

    /// Operations that are always auto-approved
    pub auto_approve_operations: HashSet<String>,

    /// Operations that are always denied
    pub auto_deny_operations: HashSet<String>,

    /// Maximum number of pending consent requests
    pub max_pending_requests: usize,

    /// How long to cache consent decisions
    pub consent_cache_duration: Duration,

    /// Whether to require consent for bulk operations
    pub require_bulk_consent: bool,

    /// Bulk operation threshold (number of devices)
    pub bulk_threshold: usize,

    /// Whether to log all consent decisions
    pub audit_all_decisions: bool,
}

impl Default for ConsentConfig {
    fn default() -> Self {
        let mut required_for_sensitivity = HashSet::new();
        required_for_sensitivity.insert(SensitivityLevel::High);
        required_for_sensitivity.insert(SensitivityLevel::Critical);

        Self {
            enabled: true,
            default_timeout: Duration::from_secs(300), // 5 minutes
            required_for_sensitivity,
            auto_approve_operations: HashSet::new(),
            auto_deny_operations: HashSet::new(),
            max_pending_requests: 10,
            consent_cache_duration: Duration::from_secs(3600), // 1 hour
            require_bulk_consent: true,
            bulk_threshold: 5,
            audit_all_decisions: true,
        }
    }
}

/// Consent manager for handling consent flows
pub struct ConsentManager {
    /// Configuration
    config: ConsentConfig,

    /// Pending consent requests
    pending_requests: RwLock<HashMap<Uuid, ConsentRequest>>,

    /// Cached consent decisions
    consent_cache: RwLock<HashMap<String, (ConsentDecision, SystemTime)>>,

    /// Consent decision history
    decision_history: RwLock<Vec<ConsentRecord>>,

    /// Channel for consent requests to UI/user
    request_sender: Option<mpsc::UnboundedSender<ConsentRequest>>,

    /// Channel for consent responses from UI/user
    response_receiver: RwLock<Option<mpsc::UnboundedReceiver<ConsentResponse>>>,
}

impl ConsentManager {
    /// Create new consent manager with default configuration
    pub fn new() -> Self {
        Self::with_config(ConsentConfig::default())
    }

    /// Create new consent manager with custom configuration
    pub fn with_config(config: ConsentConfig) -> Self {
        Self {
            config,
            pending_requests: RwLock::new(HashMap::new()),
            consent_cache: RwLock::new(HashMap::new()),
            decision_history: RwLock::new(Vec::new()),
            request_sender: None,
            response_receiver: RwLock::new(None),
        }
    }

    /// Setup consent flow channels
    pub async fn setup_channels(
        &mut self,
    ) -> (
        mpsc::UnboundedReceiver<ConsentRequest>,
        mpsc::UnboundedSender<ConsentResponse>,
    ) {
        let (request_tx, request_rx) = mpsc::unbounded_channel();
        let (response_tx, response_rx) = mpsc::unbounded_channel();

        self.request_sender = Some(request_tx);
        *self.response_receiver.write().await = Some(response_rx);

        (request_rx, response_tx)
    }

    /// Request consent for an operation
    pub async fn request_consent(
        &self,
        operation: OperationType,
        source: String,
    ) -> Result<ConsentDecision> {
        if !self.config.enabled {
            return Ok(ConsentDecision::AutoApproved {
                policy: "consent_disabled".to_string(),
            });
        }

        let sensitivity = self.classify_operation_sensitivity(&operation);

        // Check if consent is required for this sensitivity level
        if !self.config.required_for_sensitivity.contains(&sensitivity) {
            return Ok(ConsentDecision::AutoApproved {
                policy: "sensitivity_exemption".to_string(),
            });
        }

        // Check auto-approve/deny lists
        let operation_key = self.get_operation_key(&operation);
        if self.config.auto_approve_operations.contains(&operation_key) {
            return Ok(ConsentDecision::AutoApproved {
                policy: "auto_approve_list".to_string(),
            });
        }

        if self.config.auto_deny_operations.contains(&operation_key) {
            return Ok(ConsentDecision::Denied {
                reason: "Operation in auto-deny list".to_string(),
            });
        }

        // Check cached consent
        if let Some(cached_decision) = self.check_cached_consent(&operation).await {
            return Ok(cached_decision);
        }

        // Check pending request limit
        let pending_count = self.pending_requests.read().await.len();
        if pending_count >= self.config.max_pending_requests {
            return Ok(ConsentDecision::Denied {
                reason: "Too many pending consent requests".to_string(),
            });
        }

        // Create consent request
        let request = self
            .create_consent_request(operation, sensitivity, source)
            .await;
        let request_id = request.id;

        // Store pending request
        {
            let mut pending = self.pending_requests.write().await;
            pending.insert(request_id, request.clone());
        }

        // Send request to UI if channel is available
        if let Some(sender) = &self.request_sender {
            if let Err(e) = sender.send(request.clone()) {
                warn!("Failed to send consent request to UI: {}", e);
            }
        }

        // Wait for response or timeout
        let decision = self.wait_for_consent_response(request_id).await?;

        // Cache the decision if appropriate
        if matches!(decision, ConsentDecision::Approved) {
            self.cache_consent_decision(&request.operation, &decision)
                .await;
        }

        // Record the decision
        self.record_consent_decision(request, decision.clone())
            .await;

        Ok(decision)
    }

    /// Process a consent response
    pub async fn process_response(&self, response: ConsentResponse) -> Result<()> {
        let request_id = response.request_id;
        let mut pending = self.pending_requests.write().await;
        if let Some(request) = pending.remove(&response.request_id) {
            let decision = if response.approved {
                ConsentDecision::Approved
            } else {
                ConsentDecision::Denied {
                    reason: response
                        .reason
                        .clone()
                        .unwrap_or_else(|| "User denied".to_string()),
                }
            };

            // Cache decision if requested
            if response.apply_to_similar && response.approved {
                self.cache_consent_decision(&request.operation, &decision)
                    .await;
            }

            // Record the decision
            let record = ConsentRecord {
                request,
                response,
                decision,
                decision_method: DecisionMethod::UserDecision,
                execution_result: None,
            };

            let mut history = self.decision_history.write().await;
            history.push(record);

            info!("Processed consent response for request {}", request_id);
        } else {
            warn!(
                "Received response for unknown consent request: {}",
                request_id
            );
        }

        Ok(())
    }

    /// Classify operation sensitivity
    pub fn classify_operation_sensitivity(&self, operation: &OperationType) -> SensitivityLevel {
        match operation {
            OperationType::DeviceControl { command, .. } => {
                // Security-related commands are high sensitivity
                if command.contains("security")
                    || command.contains("alarm")
                    || command.contains("lock")
                {
                    SensitivityLevel::High
                } else {
                    SensitivityLevel::Medium
                }
            }
            OperationType::BulkDeviceControl { device_count, .. } => {
                if *device_count >= self.config.bulk_threshold {
                    SensitivityLevel::High
                } else {
                    SensitivityLevel::Medium
                }
            }
            OperationType::SecurityControl { .. } => SensitivityLevel::Critical,
            OperationType::SystemConfiguration { .. } => SensitivityLevel::High,
            OperationType::DataExport { .. } => SensitivityLevel::Medium,
            OperationType::ConnectionManagement { .. } => SensitivityLevel::Low,
        }
    }

    /// Get operation key for caching and comparison
    pub fn get_operation_key(&self, operation: &OperationType) -> String {
        match operation {
            OperationType::DeviceControl { command, .. } => format!("device_control:{command}"),
            OperationType::BulkDeviceControl { operation_type, .. } => {
                format!("bulk_control:{operation_type}")
            }
            OperationType::SecurityControl { action, .. } => format!("security:{action}"),
            OperationType::SystemConfiguration { setting, .. } => format!("config:{setting}"),
            OperationType::DataExport { data_type, .. } => format!("export:{data_type}"),
            OperationType::ConnectionManagement { action, .. } => format!("connection:{action}"),
        }
    }

    /// Check for cached consent decision
    async fn check_cached_consent(&self, operation: &OperationType) -> Option<ConsentDecision> {
        let cache = self.consent_cache.read().await;
        let operation_key = self.get_operation_key(operation);

        if let Some((decision, timestamp)) = cache.get(&operation_key) {
            let elapsed = SystemTime::now()
                .duration_since(*timestamp)
                .unwrap_or_default();
            if elapsed < self.config.consent_cache_duration {
                debug!("Using cached consent for operation: {}", operation_key);
                return Some(decision.clone());
            }
        }

        None
    }

    /// Cache a consent decision
    async fn cache_consent_decision(&self, operation: &OperationType, decision: &ConsentDecision) {
        let mut cache = self.consent_cache.write().await;
        let operation_key = self.get_operation_key(operation);
        cache.insert(operation_key, (decision.clone(), SystemTime::now()));
    }

    /// Create a consent request
    async fn create_consent_request(
        &self,
        operation: OperationType,
        sensitivity: SensitivityLevel,
        source: String,
    ) -> ConsentRequest {
        let (description, details, risks, impact) = self.generate_operation_description(&operation);
        let is_bulk = self.is_bulk_operation(&operation);

        ConsentRequest {
            id: Uuid::new_v4(),
            operation,
            sensitivity,
            description,
            details,
            risks,
            impact,
            is_bulk,
            created_at: SystemTime::now(),
            timeout: Some(self.config.default_timeout),
            source,
            metadata: HashMap::new(),
        }
    }

    /// Generate human-readable description for operation
    fn generate_operation_description(
        &self,
        operation: &OperationType,
    ) -> (String, String, Vec<String>, String) {
        match operation {
            OperationType::DeviceControl {
                device_name,
                command,
                ..
            } => {
                let description = format!("Control device: {device_name}");
                let details = format!("Execute command '{command}' on device '{device_name}'");
                let risks = vec![
                    "Device state will change".to_string(),
                    "May affect comfort or convenience".to_string(),
                ];
                let impact = "Single device operation".to_string();
                (description, details, risks, impact)
            }
            OperationType::BulkDeviceControl {
                device_count,
                room_name,
                operation_type,
            } => {
                let room_str = room_name.as_deref().unwrap_or("multiple rooms");
                let description = format!("Bulk control: {device_count} devices in {room_str}");
                let details = format!("Execute '{operation_type}' on {device_count} devices");
                let risks = vec![
                    "Multiple devices will change state".to_string(),
                    "May significantly affect environment".to_string(),
                    "Difficult to undo quickly".to_string(),
                ];
                let impact = format!("{device_count} devices affected");
                (description, details, risks, impact)
            }
            OperationType::SecurityControl { action, scope } => {
                let description = format!("Security operation: {action}");
                let details = format!("Execute security action '{action}' with scope '{scope}'");
                let risks = vec![
                    "Security settings will change".to_string(),
                    "May affect safety and security".to_string(),
                    "Critical system operation".to_string(),
                ];
                let impact = "Security system affected".to_string();
                (description, details, risks, impact)
            }
            OperationType::SystemConfiguration {
                setting, new_value, ..
            } => {
                let description = format!("System configuration: {setting}");
                let details = format!("Change setting '{setting}' to '{new_value}'");
                let risks = vec![
                    "System behavior will change".to_string(),
                    "May affect all connected devices".to_string(),
                ];
                let impact = "System-wide changes".to_string();
                (description, details, risks, impact)
            }
            OperationType::DataExport { data_type, scope } => {
                let description = format!("Data export: {data_type}");
                let details = format!("Export {data_type} data with scope '{scope}'");
                let risks = vec![
                    "Sensitive data will be exported".to_string(),
                    "Privacy implications".to_string(),
                ];
                let impact = "Data access operation".to_string();
                (description, details, risks, impact)
            }
            OperationType::ConnectionManagement { action, target } => {
                let description = format!("Connection: {action}");
                let details = format!("Execute '{action}' on connection to '{target}'");
                let risks = vec!["Connection state will change".to_string()];
                let impact = "Connection affected".to_string();
                (description, details, risks, impact)
            }
        }
    }

    /// Check if operation is considered bulk
    pub fn is_bulk_operation(&self, operation: &OperationType) -> bool {
        match operation {
            OperationType::BulkDeviceControl { device_count, .. } => {
                *device_count >= self.config.bulk_threshold
            }
            _ => false,
        }
    }

    /// Wait for consent response with timeout
    async fn wait_for_consent_response(&self, request_id: Uuid) -> Result<ConsentDecision> {
        // This is a simplified implementation
        // In practice, you'd want to use proper async waiting with timeout
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Check if response was processed
        let pending = self.pending_requests.read().await;
        if pending.contains_key(&request_id) {
            // Still pending, return timeout
            Ok(ConsentDecision::TimedOut)
        } else {
            // Response was processed, find it in history
            let history = self.decision_history.read().await;
            for record in history.iter().rev() {
                if record.request.id == request_id {
                    return Ok(record.decision.clone());
                }
            }
            Ok(ConsentDecision::TimedOut)
        }
    }

    /// Record consent decision for audit trail
    async fn record_consent_decision(&self, request: ConsentRequest, decision: ConsentDecision) {
        if !self.config.audit_all_decisions {
            return;
        }

        let decision_method = match &decision {
            ConsentDecision::Approved | ConsentDecision::Denied { .. } => {
                DecisionMethod::UserDecision
            }
            ConsentDecision::AutoApproved { .. } => DecisionMethod::PolicyBased,
            ConsentDecision::TimedOut => DecisionMethod::Timeout,
        };

        let record = ConsentRecord {
            request,
            response: ConsentResponse {
                request_id: Uuid::new_v4(), // Placeholder
                approved: matches!(decision, ConsentDecision::Approved),
                reason: None,
                responded_at: SystemTime::now(),
                validity_duration: None,
                apply_to_similar: false,
                user_id: None,
            },
            decision,
            decision_method,
            execution_result: None,
        };

        let mut history = self.decision_history.write().await;
        history.push(record);

        debug!("Recorded consent decision for operation");
    }

    /// Get consent statistics
    pub async fn get_statistics(&self) -> ConsentStatistics {
        let history = self.decision_history.read().await;
        let pending = self.pending_requests.read().await;

        let total_requests = history.len();
        let approved = history
            .iter()
            .filter(|r| matches!(r.decision, ConsentDecision::Approved))
            .count();
        let denied = history
            .iter()
            .filter(|r| matches!(r.decision, ConsentDecision::Denied { .. }))
            .count();
        let auto_approved = history
            .iter()
            .filter(|r| matches!(r.decision, ConsentDecision::AutoApproved { .. }))
            .count();
        let timed_out = history
            .iter()
            .filter(|r| matches!(r.decision, ConsentDecision::TimedOut))
            .count();

        ConsentStatistics {
            total_requests,
            pending_requests: pending.len(),
            approved_count: approved,
            denied_count: denied,
            auto_approved_count: auto_approved,
            timed_out_count: timed_out,
            cache_size: self.consent_cache.read().await.len(),
        }
    }

    /// Clear expired cache entries
    pub async fn cleanup_cache(&self) {
        let mut cache = self.consent_cache.write().await;
        let now = SystemTime::now();

        cache.retain(|_, (_, timestamp)| {
            now.duration_since(*timestamp).unwrap_or_default() < self.config.consent_cache_duration
        });
    }
}

impl Default for ConsentManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics about consent operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsentStatistics {
    pub total_requests: usize,
    pub pending_requests: usize,
    pub approved_count: usize,
    pub denied_count: usize,
    pub auto_approved_count: usize,
    pub timed_out_count: usize,
    pub cache_size: usize,
}

/// Helper trait for integrating consent flow with operations
#[async_trait::async_trait]
pub trait ConsentProtected {
    /// Execute operation with consent check
    async fn execute_with_consent<T>(
        &self,
        operation: OperationType,
        consent_manager: &ConsentManager,
        source: String,
        executor: impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>
            + Send,
    ) -> Result<T>;
}

#[async_trait::async_trait]
impl<S> ConsentProtected for S
where
    S: Send + Sync,
{
    async fn execute_with_consent<T>(
        &self,
        operation: OperationType,
        consent_manager: &ConsentManager,
        source: String,
        executor: impl Fn() -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<T>> + Send>>
            + Send,
    ) -> Result<T> {
        // Request consent
        let decision = consent_manager.request_consent(operation, source).await?;

        match decision {
            ConsentDecision::Approved | ConsentDecision::AutoApproved { .. } => {
                // Execute the operation
                let result = executor().await;
                result
            }
            ConsentDecision::Denied { reason } => Err(LoxoneError::consent_denied(reason)),
            ConsentDecision::TimedOut => Err(LoxoneError::consent_denied(
                "Consent request timed out".to_string(),
            )),
        }
    }
}
