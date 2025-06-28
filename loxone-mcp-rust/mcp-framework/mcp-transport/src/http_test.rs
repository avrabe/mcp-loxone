//! Integration tests for HTTP/SSE transport

#[cfg(test)]
mod tests {
    use crate::{http::HttpTransport, RequestHandler, Transport};
    use pulseengine_mcp_protocol::{Request, Response};
    use serde_json::json;
    use tokio::time::{sleep, Duration};

    // Test handler that echoes requests
    fn test_handler(
        request: Request,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response> + Send>> {
        Box::pin(async move {
            Response {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(json!({
                    "method": request.method,
                    "params": request.params,
                })),
                error: None,
            }
        })
    }

    #[tokio::test]
    async fn test_http_transport_startup() {
        // Start server on a test port
        let mut transport = HttpTransport::new(19001);
        let handler: RequestHandler = Box::new(test_handler);

        // Server should start successfully
        match transport.start(handler).await {
            Ok(_) => {}
            Err(e) => panic!("Failed to start HTTP transport: {:?}", e),
        }

        // Give server time to fully start
        sleep(Duration::from_millis(100)).await;

        // Health check should pass
        assert!(transport.health_check().await.is_ok());

        // Server should stop successfully
        assert!(transport.stop().await.is_ok());

        // Health check should fail after stop
        assert!(transport.health_check().await.is_err());
    }
}
