//! Resource Subscription System for MCP
//!
//! This module implements real-time resource change notifications for MCP clients.
//! It provides subscription management, change detection, and notification dispatch
//! across multiple transport protocols (stdio, HTTP/SSE).

pub mod detector;
pub mod dispatcher;
pub mod manager;
pub mod types;

pub use detector::ResourceChangeDetector;
pub use dispatcher::NotificationDispatcher;
pub use manager::ResourceSubscriptionManager;
pub use types::{
    ClientInfo, ClientSubscription, NotificationTarget, ResourceChange, SubscriptionEvent,
    SubscriptionFilter,
};

use crate::error::Result;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn};

/// Central subscription coordinator that manages the entire subscription lifecycle
pub struct SubscriptionCoordinator {
    /// Manages client subscriptions and resource mappings
    subscription_manager: Arc<ResourceSubscriptionManager>,

    /// Detects changes from Loxone WebSocket streams
    change_detector: Arc<ResourceChangeDetector>,

    /// Dispatches notifications to subscribed clients
    notification_dispatcher: Arc<NotificationDispatcher>,

    /// Broadcast channel for system-wide events
    system_events: broadcast::Sender<SubscriptionEvent>,
}

impl SubscriptionCoordinator {
    /// Create new subscription coordinator with all components
    pub async fn new() -> Result<Self> {
        info!("🔄 Initializing resource subscription system...");

        // Create broadcast channel for system events
        let (system_events, _) = broadcast::channel(1000);

        // Initialize core components
        let subscription_manager = Arc::new(ResourceSubscriptionManager::new());
        let change_detector = Arc::new(ResourceChangeDetector::new(system_events.clone()).await?);
        let notification_dispatcher =
            Arc::new(NotificationDispatcher::new(system_events.subscribe()));

        info!("✅ Resource subscription system initialized");

        Ok(Self {
            subscription_manager,
            change_detector,
            notification_dispatcher,
            system_events,
        })
    }

    /// Start the subscription system background tasks
    pub async fn start(&self) -> Result<()> {
        info!("🚀 Starting subscription system background tasks...");

        // Start change detection monitoring
        let change_detector = self.change_detector.clone();
        let _subscription_manager = self.subscription_manager.clone();
        let system_events = self.system_events.clone();

        tokio::spawn(async move {
            if let Err(e) = change_detector.start_monitoring().await {
                warn!("Change detector error: {}", e);
                let _ = system_events.send(SubscriptionEvent::SystemError {
                    error: e.to_string(),
                    component: "change_detector".to_string(),
                });
            }
        });

        // Start notification dispatcher
        let dispatcher = self.notification_dispatcher.clone();
        let subscription_manager_clone = self.subscription_manager.clone();

        tokio::spawn(async move {
            if let Err(e) = dispatcher
                .start_processing(subscription_manager_clone)
                .await
            {
                warn!("Notification dispatcher error: {}", e);
            }
        });

        info!("✅ Subscription system started successfully");
        Ok(())
    }

    /// Subscribe a client to resource changes
    pub async fn subscribe_client(
        &self,
        client_info: ClientInfo,
        resource_uri: String,
        filter: Option<SubscriptionFilter>,
    ) -> Result<()> {
        info!(
            "📨 Subscribing client {} to resource {}",
            client_info.id, resource_uri
        );

        let client_id = client_info.id.clone();

        // Add subscription to manager
        self.subscription_manager
            .add_subscription(client_info, resource_uri.clone(), filter)
            .await?;

        // Notify system of new subscription
        let _ = self
            .system_events
            .send(SubscriptionEvent::ClientSubscribed {
                client_id,
                resource_uri,
            });

        Ok(())
    }

    /// Unsubscribe a client from resource changes
    pub async fn unsubscribe_client(
        &self,
        client_id: String,
        resource_uri: Option<String>,
    ) -> Result<()> {
        info!(
            "📭 Unsubscribing client {} from {}",
            client_id,
            resource_uri.as_deref().unwrap_or("all resources")
        );

        self.subscription_manager
            .remove_subscription(client_id.clone(), resource_uri.clone())
            .await?;

        // Notify system of unsubscription
        let _ = self
            .system_events
            .send(SubscriptionEvent::ClientUnsubscribed {
                client_id,
                resource_uri,
            });

        Ok(())
    }

    /// Get subscription statistics for monitoring
    pub async fn get_statistics(&self) -> SubscriptionStatistics {
        let subscription_stats = self.subscription_manager.get_statistics().await;
        let detection_stats = self.change_detector.get_statistics().await;
        let dispatch_stats = self.notification_dispatcher.get_statistics().await;

        SubscriptionStatistics {
            total_subscriptions: subscription_stats.total_subscriptions,
            active_clients: subscription_stats.active_clients,
            monitored_resources: subscription_stats.monitored_resources,
            changes_detected: detection_stats.changes_detected,
            notifications_sent: dispatch_stats.notifications_sent,
            failed_notifications: dispatch_stats.failed_notifications,
        }
    }

    /// Shutdown the subscription system gracefully
    pub async fn shutdown(&self) -> Result<()> {
        info!("🛑 Shutting down subscription system...");

        // Signal shutdown to all components
        let _ = self.system_events.send(SubscriptionEvent::SystemShutdown);

        // Wait for components to shutdown gracefully
        self.change_detector.stop_monitoring().await?;
        self.notification_dispatcher.stop_processing().await?;
        self.subscription_manager.clear_all_subscriptions().await?;

        info!("✅ Subscription system shutdown complete");
        Ok(())
    }
}

/// Statistics for monitoring subscription system health
#[derive(Debug, Clone)]
pub struct SubscriptionStatistics {
    pub total_subscriptions: usize,
    pub active_clients: usize,
    pub monitored_resources: usize,
    pub changes_detected: u64,
    pub notifications_sent: u64,
    pub failed_notifications: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_subscription_coordinator_creation() {
        let coordinator = SubscriptionCoordinator::new().await;
        assert!(coordinator.is_ok());
    }

    #[tokio::test]
    async fn test_subscription_lifecycle() {
        let coordinator = SubscriptionCoordinator::new().await.unwrap();

        let client_info = ClientInfo {
            id: "test-client".to_string(),
            transport: types::ClientTransport::Stdio,
            capabilities: vec!["resources".to_string()],
            connected_at: std::time::SystemTime::now(),
        };

        // Test subscription
        let result = coordinator
            .subscribe_client(client_info, "loxone://rooms".to_string(), None)
            .await;
        assert!(result.is_ok());

        // Test unsubscription
        let result = coordinator
            .unsubscribe_client(
                "test-client".to_string(),
                Some("loxone://rooms".to_string()),
            )
            .await;
        assert!(result.is_ok());
    }
}
