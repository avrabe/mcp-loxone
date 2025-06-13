//! WASIP2 main entry point for Loxone MCP server
//!
//! This binary is specifically optimized for wasm32-wasip2 target
//! and provides the main entry point for the WASM component.

#[cfg(target_arch = "wasm32")]
use loxone_mcp_rust::wasm::wasip2::{
    initialize_wasip2_component, shutdown_wasip2_component, Wasip2ConfigLoader, Wasip2McpServer,
};
#[cfg(target_arch = "wasm32")]
use loxone_mcp_rust::wasm::WasmConfig;
#[cfg(target_arch = "wasm32")]
use std::process;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_arch = "wasm32")]
    {
        // Initialize WASM module with WASIP2 configuration
        let wasm_config = WasmConfig::for_wasip2();
        wasm_config
            .validate()
            .map_err(|e| format!("WASM configuration validation failed: {}", e))?;

        // Set up logging for WASIP2
        setup_wasip2_logging(&wasm_config)?;

        // Load server configuration from WASI config interface
        let server_config = Wasip2ConfigLoader::load_config()
            .await
            .map_err(|e| format!("Failed to load server configuration: {}", e))?;

        wasi::logging::log(
            wasi::logging::Level::Info,
            "loxone-mcp-wasip2",
            "Starting Loxone MCP server for WASIP2",
        );

        // Initialize WASIP2 component
        initialize_wasip2_component(server_config.clone())
            .await
            .map_err(|e| format!("Failed to initialize WASIP2 component: {}", e))?;

        // Create and initialize MCP server
        let mut server = Wasip2McpServer::new(server_config)
            .await
            .map_err(|e| format!("Failed to create MCP server: {}", e))?;

        server
            .initialize_client()
            .await
            .map_err(|e| format!("Failed to initialize HTTP client: {}", e))?;

        wasi::logging::log(
            wasi::logging::Level::Info,
            "loxone-mcp-wasip2",
            "MCP server initialized successfully",
        );

        // Start the main server loop
        run_server_loop(server)
            .await
            .map_err(|e| format!("Server loop failed: {}", e))?;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        eprintln!("This binary is designed for WASM32 targets only.");
        eprintln!("Use the main loxone-mcp-rust binary for native execution.");
        return Err("WASM32 binary run on non-WASM target".into());
    }

    #[cfg(target_arch = "wasm32")]
    {
        Ok(())
    }
}

/// Set up logging for WASIP2 environment
#[cfg(target_arch = "wasm32")]
fn setup_wasip2_logging(config: &WasmConfig) -> Result<(), Box<dyn std::error::Error>> {
    if config.enable_debug_logging {
        // Set up tracing for WASIP2 if debug logging is enabled
        #[cfg(feature = "debug-logging")]
        {
            use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

            // Create a WASM-compatible logging layer
            let wasm_layer = tracing_wasm::WASMLayer::new(
                tracing_wasm::WASMLayerConfigBuilder::new()
                    .set_max_level(tracing::Level::DEBUG)
                    .build(),
            );

            tracing_subscriber::registry().with(wasm_layer).init();
        }

        wasi::logging::log(
            wasi::logging::Level::Debug,
            "loxone-mcp-wasip2",
            "Debug logging enabled for WASIP2",
        );
    }

    Ok(())
}

/// Main server loop optimized for WASIP2
#[cfg(target_arch = "wasm32")]
async fn run_server_loop(server: Wasip2McpServer) -> Result<(), Box<dyn std::error::Error>> {
    wasi::logging::log(
        wasi::logging::Level::Info,
        "loxone-mcp-wasip2",
        "Entering main server loop",
    );

    // Set up signal handling for graceful shutdown
    let shutdown_signal = setup_shutdown_handler().await;

    // Main component loop
    loop {
        tokio::select! {
            // Handle shutdown signal
            _ = shutdown_signal => {
                wasi::logging::log(
                    wasi::logging::Level::Info,
                    "loxone-mcp-wasip2",
                    "Shutdown signal received",
                );
                break;
            }

            // Handle periodic maintenance
            _ = tokio::time::sleep(std::time::Duration::from_secs(60)) => {
                perform_maintenance(&server).await?;
            }
        }
    }

    // Cleanup and shutdown
    shutdown_server(server).await?;

    Ok(())
}

/// Set up shutdown signal handler for WASIP2
#[cfg(target_arch = "wasm32")]
async fn setup_shutdown_handler() -> tokio::sync::oneshot::Receiver<()> {
    let (tx, rx) = tokio::sync::oneshot::channel();

    // In WASIP2, we'll use a simple timeout-based shutdown for now
    // In a real implementation, this would listen for WASI signals
    tokio::spawn(async move {
        // Wait for a long time or until external shutdown
        tokio::time::sleep(std::time::Duration::from_secs(86400)).await; // 24 hours
        let _ = tx.send(());
    });

    rx
}

/// Perform periodic maintenance tasks
#[cfg(target_arch = "wasm32")]
async fn perform_maintenance(server: &Wasip2McpServer) -> Result<(), Box<dyn std::error::Error>> {
    // Get and log metrics
    if let Ok(metrics) = Wasip2McpServer::get_metrics() {
        wasi::logging::log(
            wasi::logging::Level::Debug,
            "loxone-mcp-wasip2",
            &format!(
                "Server metrics: requests={}, failures={}",
                metrics.requests_total, metrics.requests_failed
            ),
        );
    }

    // Perform memory optimization
    #[cfg(feature = "debug-logging")]
    {
        use loxone_mcp_rust::wasm::optimizations::WasmMemoryManager;
        WasmMemoryManager::optimize_memory();
    }

    Ok(())
}

/// Graceful server shutdown
#[cfg(target_arch = "wasm32")]
async fn shutdown_server(server: Wasip2McpServer) -> Result<(), Box<dyn std::error::Error>> {
    wasi::logging::log(
        wasi::logging::Level::Info,
        "loxone-mcp-wasip2",
        "Starting graceful shutdown",
    );

    // Shutdown WASIP2 component
    shutdown_wasip2_component()
        .await
        .map_err(|e| format!("Component shutdown failed: {}", e))?;

    wasi::logging::log(
        wasi::logging::Level::Info,
        "loxone-mcp-wasip2",
        "Server shutdown completed",
    );

    Ok(())
}

/// Handle panics in WASIP2 environment
#[cfg(target_arch = "wasm32")]
fn setup_panic_handler() {
    std::panic::set_hook(Box::new(|panic_info| {
        let message = if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            s
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            s
        } else {
            "Unknown panic occurred"
        };

        wasi::logging::log(
            wasi::logging::Level::Error,
            "loxone-mcp-wasip2",
            &format!("PANIC: {}", message),
        );

        // Exit with error code
        process::exit(1);
    }));
}

/// Component initialization for WASIP2
#[cfg(target_arch = "wasm32")]
#[no_mangle]
pub extern "C" fn _start() {
    setup_panic_handler();

    // Create runtime and run main function
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create async runtime");

    if let Err(e) = rt.block_on(main()) {
        wasi::logging::log(
            wasi::logging::Level::Error,
            "loxone-mcp-wasip2",
            &format!("Application failed: {}", e),
        );
        process::exit(1);
    }
}

#[cfg(all(test, target_arch = "wasm32"))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wasip2_config_loading() {
        // Test configuration loading
        let config = WasmConfig::for_wasip2();
        assert!(config.validate().is_ok());
        assert_eq!(config.target, loxone_mcp_rust::wasm::WasmTarget::Wasip2);
    }

    #[tokio::test]
    async fn test_server_maintenance() {
        // Test that maintenance doesn't panic
        let config = loxone_mcp_rust::config::ServerConfig::default();
        if let Ok(server) = Wasip2McpServer::new(config).await {
            let result = perform_maintenance(&server).await;
            assert!(result.is_ok());
        }
    }
}
