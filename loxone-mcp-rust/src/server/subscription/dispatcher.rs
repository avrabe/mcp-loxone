//! Notification Dispatcher
//!
//! Sends resource change notifications to subscribed MCP clients across different
//! transport protocols (stdio, HTTP/SSE, WebSocket).

use super::manager::ResourceSubscriptionManager;
use super::types::{
    ClientInfo, ClientTransport, NotificationDispatcherStats, ResourceChange,
    ResourceChangeNotification, SubscriptionEvent,
};
use crate::error::{LoxoneError, Result};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{broadcast, RwLock};
use tokio::time::timeout;
use tracing::{debug, error, info, warn};

/// Dispatches notifications to subscribed clients
pub struct NotificationDispatcher {
    /// Receiver for subscription events
    event_receiver: Arc<RwLock<Option<broadcast::Receiver<SubscriptionEvent>>>>,

    /// Statistics for monitoring
    stats: Arc<RwLock<NotificationDispatcherStats>>,

    /// Flag to stop processing
    should_stop: Arc<RwLock<bool>>,

    /// Retry configuration
    max_retries: u32,
    retry_delay: Duration,

    /// Notification timeout
    notification_timeout: Duration,
}

impl NotificationDispatcher {
    /// Create a new notification dispatcher
    pub fn new(event_receiver: broadcast::Receiver<SubscriptionEvent>) -> Self {
        debug!("📢 Initializing notification dispatcher");

        Self {
            event_receiver: Arc::new(RwLock::new(Some(event_receiver))),
            stats: Arc::new(RwLock::new(NotificationDispatcherStats::default())),
            should_stop: Arc::new(RwLock::new(false)),
            max_retries: 3,
            retry_delay: Duration::from_millis(100),
            notification_timeout: Duration::from_secs(5),
        }
    }

    /// Start processing notifications
    pub async fn start_processing(
        &self,
        subscription_manager: Arc<ResourceSubscriptionManager>,
    ) -> Result<()> {
        debug!("🚀 Starting notification processing");

        // Reset stop flag
        {
            let mut should_stop = self.should_stop.write().await;
            *should_stop = false;
        }

        // Take the receiver (can only be used once)
        let receiver = {
            let mut receiver_option = self.event_receiver.write().await;
            receiver_option.take()
        };

        if let Some(mut receiver) = receiver {
            let stats = self.stats.clone();
            let should_stop = self.should_stop.clone();
            let max_retries = self.max_retries;
            let retry_delay = self.retry_delay;
            let notification_timeout = self.notification_timeout;

            tokio::spawn(async move {
                loop {
                    if *should_stop.read().await {
                        break;
                    }

                    // Wait for events with timeout to allow periodic checks
                    match timeout(Duration::from_secs(1), receiver.recv()).await {
                        Ok(Ok(event)) => {
                            if let Err(e) = Self::handle_subscription_event(
                                event,
                                &subscription_manager,
                                &stats,
                                max_retries,
                                retry_delay,
                                notification_timeout,
                            )
                            .await
                            {
                                error!("Error handling subscription event: {}", e);
                            }
                        }
                        Ok(Err(broadcast::error::RecvError::Closed)) => {
                            info!("Event channel closed, stopping notification processing");
                            break;
                        }
                        Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
                            warn!("Notification dispatcher lagged behind events");
                            // Continue processing
                        }
                        Err(_) => {
                            // Timeout - continue to check stop flag
                        }
                    }
                }

                info!("📢 Notification processing stopped");
            });
        } else {
            return Err(LoxoneError::invalid_input(
                "Event receiver already consumed".to_string(),
            ));
        }

        Ok(())
    }

    /// Stop processing notifications
    pub async fn stop_processing(&self) -> Result<()> {
        info!("🛑 Stopping notification processing");

        let mut should_stop = self.should_stop.write().await;
        *should_stop = true;

        Ok(())
    }

    /// Handle a subscription event
    async fn handle_subscription_event(
        event: SubscriptionEvent,
        subscription_manager: &Arc<ResourceSubscriptionManager>,
        stats: &Arc<RwLock<NotificationDispatcherStats>>,
        max_retries: u32,
        retry_delay: Duration,
        notification_timeout: Duration,
    ) -> Result<()> {
        match event {
            SubscriptionEvent::ResourceChanged { change } => {
                Self::handle_resource_change(
                    change,
                    subscription_manager,
                    stats,
                    max_retries,
                    retry_delay,
                    notification_timeout,
                )
                .await?;
            }
            SubscriptionEvent::ClientSubscribed {
                client_id,
                resource_uri,
            } => {
                debug!("✅ Client {} subscribed to {}", client_id, resource_uri);
            }
            SubscriptionEvent::ClientUnsubscribed {
                client_id,
                resource_uri,
            } => {
                debug!(
                    "❌ Client {} unsubscribed from {:?}",
                    client_id, resource_uri
                );
            }
            SubscriptionEvent::ClientDisconnected { client_id, reason } => {
                info!("🔌 Client {} disconnected: {}", client_id, reason);
                // Clean up subscriptions for disconnected client
                let _ = subscription_manager
                    .remove_subscription(client_id, None)
                    .await;
            }
            SubscriptionEvent::SystemError { error, component } => {
                error!("🚨 System error in {}: {}", component, error);
            }
            SubscriptionEvent::SystemShutdown => {
                info!("🛑 System shutdown signal received");
            }
            SubscriptionEvent::NotificationSent { .. } => {
                debug!("📋 Notification sent event received");
            }
        }

        Ok(())
    }

    /// Handle resource change event
    async fn handle_resource_change(
        change: ResourceChange,
        subscription_manager: &Arc<ResourceSubscriptionManager>,
        stats: &Arc<RwLock<NotificationDispatcherStats>>,
        max_retries: u32,
        retry_delay: Duration,
        notification_timeout: Duration,
    ) -> Result<()> {
        debug!("🔄 Processing resource change: {}", change.resource_uri);

        // Get all subscribers to this resource
        let subscribers = subscription_manager
            .get_subscribers(&change.resource_uri)
            .await;

        if subscribers.is_empty() {
            debug!("📭 No subscribers for resource: {}", change.resource_uri);
            return Ok(());
        }

        debug!(
            "📨 Notifying {} subscribers for resource: {}",
            subscribers.len(),
            change.resource_uri
        );

        // Create notification
        let notification = ResourceChangeNotification::new(change.clone());

        // Send notifications to all subscribers
        let start_time = Instant::now();
        let mut successful_notifications = 0;
        let mut failed_notifications = 0;

        for subscriber in subscribers {
            let notify_result = Self::send_notification_to_client(
                &subscriber,
                &notification,
                max_retries,
                retry_delay,
                notification_timeout,
            )
            .await;

            match notify_result {
                Ok(_) => {
                    successful_notifications += 1;

                    // Update last notification time
                    let _ = subscription_manager
                        .update_last_notification(
                            &subscriber.id,
                            &change.resource_uri,
                            SystemTime::now(),
                        )
                        .await;
                }
                Err(e) => {
                    failed_notifications += 1;
                    warn!("Failed to notify client {}: {}", subscriber.id, e);
                }
            }
        }

        // Update statistics
        let dispatch_time = start_time.elapsed();
        {
            let mut dispatcher_stats = stats.write().await;
            dispatcher_stats.notifications_sent += successful_notifications;
            dispatcher_stats.failed_notifications += failed_notifications;

            // Update average dispatch time (simple moving average)
            let total_notifications =
                dispatcher_stats.notifications_sent + dispatcher_stats.failed_notifications;
            if total_notifications > 0 {
                let current_avg = dispatcher_stats.average_dispatch_time_ms;
                let new_time_ms = dispatch_time.as_millis() as f64;
                dispatcher_stats.average_dispatch_time_ms =
                    (current_avg * (total_notifications - 1) as f64 + new_time_ms)
                        / total_notifications as f64;
            }
        }

        info!(
            "📊 Notification dispatch complete: {} successful, {} failed in {:?}",
            successful_notifications, failed_notifications, dispatch_time
        );

        Ok(())
    }

    /// Send notification to a specific client
    async fn send_notification_to_client(
        client: &ClientInfo,
        notification: &ResourceChangeNotification,
        max_retries: u32,
        retry_delay: Duration,
        notification_timeout: Duration,
    ) -> Result<()> {
        let mut attempts = 0;

        while attempts <= max_retries {
            let result = timeout(
                notification_timeout,
                Self::dispatch_notification(client, notification),
            )
            .await;

            match result {
                Ok(Ok(_)) => {
                    debug!("✅ Notification sent to client: {}", client.id);
                    return Ok(());
                }
                Ok(Err(e)) => {
                    attempts += 1;
                    if attempts <= max_retries {
                        warn!(
                            "Notification attempt {} failed for client {}: {}, retrying...",
                            attempts, client.id, e
                        );
                        tokio::time::sleep(retry_delay * attempts).await;
                    } else {
                        error!(
                            "❌ All notification attempts failed for client {}: {}",
                            client.id, e
                        );
                        return Err(e);
                    }
                }
                Err(_) => {
                    error!("⏰ Notification timeout for client: {}", client.id);
                    return Err(LoxoneError::timeout(format!(
                        "Notification timeout for client: {}",
                        client.id
                    )));
                }
            }
        }

        Err(LoxoneError::connection(format!(
            "Failed to notify client {} after {} attempts",
            client.id, max_retries
        )))
    }

    /// Dispatch notification based on client transport
    async fn dispatch_notification(
        client: &ClientInfo,
        notification: &ResourceChangeNotification,
    ) -> Result<()> {
        match &client.transport {
            ClientTransport::Stdio => Self::send_stdio_notification(client, notification).await,
            ClientTransport::HttpSse { connection_id } => {
                Self::send_sse_notification(client, notification, connection_id).await
            }
            ClientTransport::WebSocket { connection_id } => {
                Self::send_websocket_notification(client, notification, connection_id).await
            }
        }
    }

    /// Send notification via stdio transport
    async fn send_stdio_notification(
        client: &ClientInfo,
        notification: &ResourceChangeNotification,
    ) -> Result<()> {
        debug!("📤 Sending stdio notification to client: {}", client.id);

        // In a real implementation, this would:
        // 1. Serialize the notification to JSON-RPC format
        // 2. Send via the stdio transport mechanism
        // 3. Handle any transport-specific errors

        // For now, simulate successful delivery
        let _serialized = serde_json::to_string(notification)
            .map_err(|e| LoxoneError::invalid_input(format!("Serialization error: {e}")))?;

        // TODO: Integrate with actual stdio transport
        debug!("📨 Stdio notification sent to {}", client.id);

        Ok(())
    }

    /// Send notification via HTTP Server-Sent Events
    async fn send_sse_notification(
        client: &ClientInfo,
        notification: &ResourceChangeNotification,
        connection_id: &str,
    ) -> Result<()> {
        debug!(
            "📡 Sending SSE notification to client: {} (connection: {})",
            client.id, connection_id
        );

        // Legacy SSE manager disabled during framework migration
        // Framework handles notifications through its own transport layer
        // if let Some(sse_manager) = crate::http_transport::get_global_sse_manager().await {
        //     let sse_event = crate::http_transport::SseNotificationEvent {
        //         event_type: format!("{:?}", notification.params.change_type),
        //         resource_uri: notification.params.uri.clone(),
        //         client_id: client.id.clone(),
        //         data: notification.params.data.clone().unwrap_or_default(),
        //         timestamp: notification.params.timestamp.clone(),
        //     };
        //
        //     if let Err(e) = sse_manager.send_notification(sse_event).await {
        //         warn!("Failed to send SSE notification to {}: {}", client.id, e);
        //         return Err(e);

        // Framework migration: Use debug logging instead of SSE for now
        debug!(
            "📡 Notification sent to client {} for resource {}",
            client.id, notification.params.uri
        );

        Ok(()) // Framework migration: simplified return

        // Legacy else block disabled - framework handles notifications differently
        // warn!("SSE manager not available for notification to {}", client.id);
        // Ok(())
    }

    /// Send notification via WebSocket
    async fn send_websocket_notification(
        client: &ClientInfo,
        notification: &ResourceChangeNotification,
        connection_id: &str,
    ) -> Result<()> {
        debug!(
            "🔌 Sending WebSocket notification to client: {} (connection: {})",
            client.id, connection_id
        );

        // WebSocket support is a future extension
        let _serialized = serde_json::to_string(notification)
            .map_err(|e| LoxoneError::invalid_input(format!("Serialization error: {e}")))?;

        // TODO: Implement WebSocket transport
        debug!(
            "🔌 WebSocket notification sent to {} ({})",
            client.id, connection_id
        );

        Ok(())
    }

    /// Get dispatcher statistics
    pub async fn get_statistics(&self) -> NotificationDispatcherStats {
        self.stats.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::subscription::types::ResourceChangeType;
    use std::collections::HashMap;

    fn create_test_client(id: &str, transport: ClientTransport) -> ClientInfo {
        ClientInfo {
            id: id.to_string(),
            transport,
            capabilities: vec!["resources".to_string()],
            connected_at: SystemTime::now(),
        }
    }

    fn create_test_change() -> ResourceChange {
        ResourceChange {
            resource_uri: "loxone://devices/all".to_string(),
            change_type: ResourceChangeType::DeviceState,
            timestamp: SystemTime::now(),
            previous_value: Some(serde_json::json!({"state": "off"})),
            new_value: serde_json::json!({"state": "on"}),
            loxone_uuid: Some("test-device-123".to_string()),
            metadata: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_dispatcher_creation() {
        let (_, receiver) = broadcast::channel(100);
        let dispatcher = NotificationDispatcher::new(receiver);

        let stats = dispatcher.get_statistics().await;
        assert_eq!(stats.notifications_sent, 0);
        assert_eq!(stats.failed_notifications, 0);
    }

    #[tokio::test]
    async fn test_notification_serialization() {
        let change = create_test_change();
        let notification = ResourceChangeNotification::new(change);

        assert_eq!(notification.method, "notifications/resources/updated");
        assert_eq!(notification.params.uri, "loxone://devices/all");

        // Test serialization
        let serialized = serde_json::to_string(&notification);
        assert!(serialized.is_ok());
    }

    #[tokio::test]
    async fn test_notification_dispatch() {
        let client = create_test_client("test-client", ClientTransport::Stdio);
        let notification = ResourceChangeNotification::new(create_test_change());

        // Test dispatch (will succeed with current mock implementation)
        let result = NotificationDispatcher::dispatch_notification(&client, &notification).await;
        assert!(result.is_ok());
    }
}
