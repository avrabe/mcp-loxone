//! Resource Subscription Manager
//!
//! Manages client subscriptions to MCP resources, handles subscription lifecycle,
//! and provides efficient lookups for notification targeting.

use super::types::{
    ClientInfo, ClientSubscription, ClientTransport, SubscriptionFilter, SubscriptionManagerStats,
};
use crate::error::{LoxoneError, Result};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Manages all client subscriptions to resources
pub struct ResourceSubscriptionManager {
    /// Map of client ID to their subscriptions
    client_subscriptions: Arc<RwLock<HashMap<String, Vec<ClientSubscription>>>>,

    /// Map of resource URI to list of subscribed client IDs
    resource_subscribers: Arc<RwLock<HashMap<String, HashSet<String>>>>,

    /// Map of client ID to client info
    client_info: Arc<RwLock<HashMap<String, ClientInfo>>>,

    /// Statistics for monitoring
    stats: Arc<RwLock<SubscriptionManagerStats>>,
}

impl ResourceSubscriptionManager {
    /// Create a new subscription manager
    pub fn new() -> Self {
        info!("📊 Initializing resource subscription manager");

        Self {
            client_subscriptions: Arc::new(RwLock::new(HashMap::new())),
            resource_subscribers: Arc::new(RwLock::new(HashMap::new())),
            client_info: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(SubscriptionManagerStats::default())),
        }
    }

    /// Add a new subscription for a client
    pub async fn add_subscription(
        &self,
        client: ClientInfo,
        resource_uri: String,
        filter: Option<SubscriptionFilter>,
    ) -> Result<()> {
        let client_id = client.id.clone();

        debug!(
            "📨 Adding subscription: client={}, resource={}",
            client_id, resource_uri
        );

        // Validate the resource URI
        self.validate_resource_uri(&resource_uri)?;

        let subscription = ClientSubscription {
            client: client.clone(),
            resource_uri: resource_uri.clone(),
            filter,
            subscribed_at: SystemTime::now(),
            last_notification: None,
        };

        // Add to client subscriptions
        {
            let mut client_subs = self.client_subscriptions.write().await;
            client_subs
                .entry(client_id.clone())
                .or_insert_with(Vec::new)
                .push(subscription);
        }

        // Add to resource subscribers index
        {
            let mut resource_subs = self.resource_subscribers.write().await;
            resource_subs
                .entry(resource_uri.clone())
                .or_insert_with(HashSet::new)
                .insert(client_id.clone());
        }

        // Store client info
        {
            let mut clients = self.client_info.write().await;
            clients.insert(client_id.clone(), client);
        }

        // Update statistics
        self.update_stats().await;

        info!(
            "✅ Subscription added: client={}, resource={}",
            client_id, resource_uri
        );
        Ok(())
    }

    /// Remove a subscription for a client
    pub async fn remove_subscription(
        &self,
        client_id: String,
        resource_uri: Option<String>,
    ) -> Result<()> {
        debug!(
            "📭 Removing subscription: client={}, resource={:?}",
            client_id, resource_uri
        );

        match resource_uri {
            Some(uri) => {
                // Remove specific resource subscription
                self.remove_specific_subscription(&client_id, &uri).await?;
            }
            None => {
                // Remove all subscriptions for the client
                self.remove_all_client_subscriptions(&client_id).await?;
            }
        }

        // Update statistics
        self.update_stats().await;

        info!("✅ Subscription removed: client={}", client_id);
        Ok(())
    }

    /// Remove a specific resource subscription for a client
    async fn remove_specific_subscription(
        &self,
        client_id: &str,
        resource_uri: &str,
    ) -> Result<()> {
        // Remove from client subscriptions
        {
            let mut client_subs = self.client_subscriptions.write().await;
            if let Some(subscriptions) = client_subs.get_mut(client_id) {
                subscriptions.retain(|sub| sub.resource_uri != resource_uri);

                // Remove client entry if no subscriptions left
                if subscriptions.is_empty() {
                    client_subs.remove(client_id);
                }
            }
        }

        // Remove from resource subscribers index
        {
            let mut resource_subs = self.resource_subscribers.write().await;
            if let Some(subscribers) = resource_subs.get_mut(resource_uri) {
                subscribers.remove(client_id);

                // Remove resource entry if no subscribers left
                if subscribers.is_empty() {
                    resource_subs.remove(resource_uri);
                }
            }
        }

        Ok(())
    }

    /// Remove all subscriptions for a client
    async fn remove_all_client_subscriptions(&self, client_id: &str) -> Result<()> {
        // Get all resource URIs this client is subscribed to
        let resource_uris: Vec<String> = {
            let client_subs = self.client_subscriptions.read().await;
            client_subs
                .get(client_id)
                .map(|subs| subs.iter().map(|sub| sub.resource_uri.clone()).collect())
                .unwrap_or_default()
        };

        // Remove from client subscriptions
        {
            let mut client_subs = self.client_subscriptions.write().await;
            client_subs.remove(client_id);
        }

        // Remove from resource subscribers index
        {
            let mut resource_subs = self.resource_subscribers.write().await;
            for uri in resource_uris {
                if let Some(subscribers) = resource_subs.get_mut(&uri) {
                    subscribers.remove(client_id);

                    // Remove resource entry if no subscribers left
                    if subscribers.is_empty() {
                        resource_subs.remove(&uri);
                    }
                }
            }
        }

        // Remove client info
        {
            let mut clients = self.client_info.write().await;
            clients.remove(client_id);
        }

        Ok(())
    }

    /// Get all clients subscribed to a specific resource
    pub async fn get_subscribers(&self, resource_uri: &str) -> Vec<ClientInfo> {
        let resource_subs = self.resource_subscribers.read().await;
        let client_info = self.client_info.read().await;

        if let Some(subscriber_ids) = resource_subs.get(resource_uri) {
            subscriber_ids
                .iter()
                .filter_map(|client_id| client_info.get(client_id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Get all subscriptions for a specific client
    pub async fn get_client_subscriptions(&self, client_id: &str) -> Vec<ClientSubscription> {
        let client_subs = self.client_subscriptions.read().await;
        client_subs.get(client_id).cloned().unwrap_or_default()
    }

    /// Get subscription for a specific client and resource
    pub async fn get_subscription(
        &self,
        client_id: &str,
        resource_uri: &str,
    ) -> Option<ClientSubscription> {
        let client_subs = self.client_subscriptions.read().await;
        client_subs
            .get(client_id)?
            .iter()
            .find(|sub| sub.resource_uri == resource_uri)
            .cloned()
    }

    /// Update the last notification time for a subscription
    pub async fn update_last_notification(
        &self,
        client_id: &str,
        resource_uri: &str,
        timestamp: SystemTime,
    ) -> Result<()> {
        let mut client_subs = self.client_subscriptions.write().await;
        if let Some(subscriptions) = client_subs.get_mut(client_id) {
            if let Some(subscription) = subscriptions
                .iter_mut()
                .find(|sub| sub.resource_uri == resource_uri)
            {
                subscription.last_notification = Some(timestamp);
            }
        }
        Ok(())
    }

    /// Get all monitored resource URIs
    pub async fn get_monitored_resources(&self) -> HashSet<String> {
        let resource_subs = self.resource_subscribers.read().await;
        resource_subs.keys().cloned().collect()
    }

    /// Check if a resource has any subscribers
    pub async fn has_subscribers(&self, resource_uri: &str) -> bool {
        let resource_subs = self.resource_subscribers.read().await;
        resource_subs
            .get(resource_uri)
            .map(|subscribers| !subscribers.is_empty())
            .unwrap_or(false)
    }

    /// Get clients by transport type
    pub async fn get_clients_by_transport(
        &self,
        transport_type: &ClientTransport,
    ) -> Vec<ClientInfo> {
        let client_info = self.client_info.read().await;
        client_info
            .values()
            .filter(|client| {
                std::mem::discriminant(&client.transport) == std::mem::discriminant(transport_type)
            })
            .cloned()
            .collect()
    }

    /// Clear all subscriptions (for shutdown)
    pub async fn clear_all_subscriptions(&self) -> Result<()> {
        info!("🧹 Clearing all subscriptions");

        let mut client_subs = self.client_subscriptions.write().await;
        let mut resource_subs = self.resource_subscribers.write().await;
        let mut clients = self.client_info.write().await;

        client_subs.clear();
        resource_subs.clear();
        clients.clear();

        // Reset statistics
        {
            let mut stats = self.stats.write().await;
            *stats = SubscriptionManagerStats::default();
        }

        info!("✅ All subscriptions cleared");
        Ok(())
    }

    /// Get subscription statistics
    pub async fn get_statistics(&self) -> SubscriptionManagerStats {
        self.stats.read().await.clone()
    }

    /// Update internal statistics
    async fn update_stats(&self) {
        let client_subs = self.client_subscriptions.read().await;
        let resource_subs = self.resource_subscribers.read().await;
        let client_info = self.client_info.read().await;

        let mut transport_counts = HashMap::new();
        for client in client_info.values() {
            let transport_name = match &client.transport {
                ClientTransport::Stdio => "stdio",
                ClientTransport::HttpSse { .. } => "http_sse",
                ClientTransport::WebSocket { .. } => "websocket",
            };
            *transport_counts
                .entry(transport_name.to_string())
                .or_insert(0) += 1;
        }

        let total_subscriptions = client_subs.values().map(|subs| subs.len()).sum();

        let mut stats = self.stats.write().await;
        stats.total_subscriptions = total_subscriptions;
        stats.active_clients = client_info.len();
        stats.monitored_resources = resource_subs.len();
        stats.subscriptions_by_transport = transport_counts;
    }

    /// Validate that a resource URI is valid for subscription
    fn validate_resource_uri(&self, uri: &str) -> Result<()> {
        if !uri.starts_with("loxone://") {
            return Err(LoxoneError::invalid_input(format!(
                "Invalid resource URI scheme: {uri}. Must start with 'loxone://'"
            )));
        }

        // Additional validation could be added here for specific URI patterns

        Ok(())
    }
}

impl Default for ResourceSubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_client(id: &str) -> ClientInfo {
        ClientInfo {
            id: id.to_string(),
            transport: ClientTransport::Stdio,
            capabilities: vec!["resources".to_string()],
            connected_at: SystemTime::now(),
        }
    }

    #[tokio::test]
    async fn test_subscription_lifecycle() {
        let manager = ResourceSubscriptionManager::new();
        let client = create_test_client("test-client");
        let resource_uri = "loxone://rooms".to_string();

        // Test adding subscription
        let result = manager
            .add_subscription(client, resource_uri.clone(), None)
            .await;
        assert!(result.is_ok());

        // Test getting subscribers
        let subscribers = manager.get_subscribers(&resource_uri).await;
        assert_eq!(subscribers.len(), 1);
        assert_eq!(subscribers[0].id, "test-client");

        // Test removing subscription
        let result = manager
            .remove_subscription("test-client".to_string(), Some(resource_uri.clone()))
            .await;
        assert!(result.is_ok());

        // Verify subscription removed
        let subscribers = manager.get_subscribers(&resource_uri).await;
        assert_eq!(subscribers.len(), 0);
    }

    #[tokio::test]
    async fn test_multiple_subscriptions() {
        let manager = ResourceSubscriptionManager::new();

        // Add multiple clients
        let client1 = create_test_client("client-1");
        let client2 = create_test_client("client-2");
        let resource_uri = "loxone://devices/all".to_string();

        manager
            .add_subscription(client1, resource_uri.clone(), None)
            .await
            .unwrap();
        manager
            .add_subscription(client2, resource_uri.clone(), None)
            .await
            .unwrap();

        // Test getting all subscribers
        let subscribers = manager.get_subscribers(&resource_uri).await;
        assert_eq!(subscribers.len(), 2);

        // Test statistics
        let stats = manager.get_statistics().await;
        assert_eq!(stats.total_subscriptions, 2);
        assert_eq!(stats.active_clients, 2);
        assert_eq!(stats.monitored_resources, 1);
    }

    #[tokio::test]
    async fn test_invalid_resource_uri() {
        let manager = ResourceSubscriptionManager::new();
        let client = create_test_client("test-client");

        // Test invalid URI scheme
        let result = manager
            .add_subscription(client, "http://invalid".to_string(), None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_client_cleanup() {
        let manager = ResourceSubscriptionManager::new();
        let client = create_test_client("test-client");

        // Add multiple subscriptions for one client
        manager
            .add_subscription(client.clone(), "loxone://rooms".to_string(), None)
            .await
            .unwrap();
        manager
            .add_subscription(client, "loxone://devices/all".to_string(), None)
            .await
            .unwrap();

        // Remove all subscriptions for client
        manager
            .remove_subscription("test-client".to_string(), None)
            .await
            .unwrap();

        // Verify all subscriptions removed
        let subscriptions = manager.get_client_subscriptions("test-client").await;
        assert_eq!(subscriptions.len(), 0);

        let stats = manager.get_statistics().await;
        assert_eq!(stats.total_subscriptions, 0);
        assert_eq!(stats.active_clients, 0);
    }
}
