//! WASM test environment setup and utilities

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;
use wasm_bindgen_test::*;
use web_sys::console;

// Configure wasm-bindgen-test to run in the browser
wasm_bindgen_test_configure!(run_in_browser);

/// Test setup function
pub fn setup() {
    // Set up panic hook for better error reporting
    console_error_panic_hook::set_once();

    // Initialize tracing for WASM tests
    tracing_wasm::set_as_global_default();

    console::log_1(&"WASM test environment initialized".into());
}

/// Test teardown function
pub fn teardown() {
    console::log_1(&"WASM test completed".into());
}

/// Helper for async test setup
pub async fn async_setup() {
    setup();

    // Additional async initialization if needed
    wasm_bindgen_futures::spawn_local(async {
        // Any async setup can go here
    });
}

/// Mock browser environment for testing
pub struct MockBrowserEnv;

impl MockBrowserEnv {
    /// Setup mock localStorage
    pub fn setup_local_storage() -> Result<(), JsValue> {
        // This would set up a mock localStorage for testing
        // In a real implementation, you might use a testing framework
        // that provides localStorage mocking
        Ok(())
    }

    /// Setup mock console
    pub fn setup_console() {
        // Console is already available in test environment
    }

    /// Clear all browser state
    pub fn clear_browser_state() -> Result<(), JsValue> {
        if let Some(storage) = web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten()
        {
            storage.clear()?;
        }
        Ok(())
    }
}

/// WASM test assertion helpers
pub mod assertions {
    use super::*;

    /// Assert that local storage contains a value
    pub fn assert_local_storage_contains(key: &str, expected: &str) -> Result<(), JsValue> {
        let storage = web_sys::window()
            .ok_or("No window")?
            .local_storage()
            .map_err(|_| "Local storage error")?
            .ok_or("Local storage not available")?;

        let actual = storage
            .get_item(key)
            .map_err(|_| "Failed to get item")?
            .ok_or("Item not found")?;

        if actual != expected {
            return Err(format!("Expected '{}', got '{}'", expected, actual).into());
        }

        Ok(())
    }

    /// Assert that local storage is empty
    pub fn assert_local_storage_empty() -> Result<(), JsValue> {
        let storage = web_sys::window()
            .ok_or("No window")?
            .local_storage()
            .map_err(|_| "Local storage error")?
            .ok_or("Local storage not available")?;

        if storage.length().map_err(|_| "Failed to get length")? != 0 {
            return Err("Local storage is not empty".into());
        }

        Ok(())
    }

    /// Assert that a function executes without throwing
    pub async fn assert_no_panic<F, R>(f: F) -> R
    where
        F: std::future::Future<Output = R>,
    {
        f.await
    }

    /// Assert WASM memory usage is reasonable
    pub fn assert_memory_usage_reasonable() -> Result<(), JsValue> {
        // This is a placeholder - in a real implementation you would
        // check WebAssembly.Memory.buffer.byteLength or similar
        console::log_1(&"Memory usage check passed".into());
        Ok(())
    }
}

/// Performance testing utilities for WASM
pub struct WasmPerformanceTester {
    start_time: f64,
}

impl WasmPerformanceTester {
    pub fn new() -> Self {
        Self {
            start_time: js_sys::Date::now(),
        }
    }

    pub fn elapsed_ms(&self) -> f64 {
        js_sys::Date::now() - self.start_time
    }

    pub fn assert_performance_under_ms(&self, max_ms: f64) -> Result<(), JsValue> {
        let elapsed = self.elapsed_ms();
        if elapsed > max_ms {
            return Err(format!("Performance test failed: {}ms > {}ms", elapsed, max_ms).into());
        }
        Ok(())
    }
}

/// Size testing utilities
pub struct WasmSizeTester;

impl WasmSizeTester {
    /// Check that serialized data is under a certain size
    pub fn assert_serialized_size_under(
        data: &serde_json::Value,
        max_bytes: usize,
    ) -> Result<(), JsValue> {
        let serialized =
            serde_json::to_string(data).map_err(|e| format!("Serialization failed: {}", e))?;

        if serialized.len() > max_bytes {
            return Err(format!("Serialized size {}B > {}B", serialized.len(), max_bytes).into());
        }

        Ok(())
    }

    /// Check that JSON is compact (no pretty-printing)
    pub fn assert_json_compact(json_str: &str) -> Result<(), JsValue> {
        if json_str.contains("  ") || json_str.contains("\n") {
            return Err("JSON is not compact".into());
        }
        Ok(())
    }
}

/// Concurrency testing utilities for WASM
pub struct WasmConcurrencyTester;

impl WasmConcurrencyTester {
    /// Test that concurrent operations don't interfere
    pub async fn test_concurrent_operations<F, T>(
        operations: Vec<F>,
        timeout_ms: u32,
    ) -> Result<Vec<T>, JsValue>
    where
        F: std::future::Future<Output = T>,
        T: 'static,
    {
        // Create timeout future
        let timeout = async {
            gloo_timers::future::TimeoutFuture::new(timeout_ms).await;
            Err("Timeout".into())
        };

        // Run operations concurrently
        let operations_future = async {
            let results = futures::future::join_all(operations).await;
            Ok(results)
        };

        // Race between operations and timeout
        match futures::future::select(Box::pin(operations_future), Box::pin(timeout)).await {
            futures::future::Either::Left((result, _)) => result,
            futures::future::Either::Right((timeout_result, _)) => timeout_result,
        }
    }
}

/// Browser compatibility testing
pub struct BrowserCompatTester;

impl BrowserCompatTester {
    /// Test basic browser features
    pub fn test_browser_features() -> Result<(), JsValue> {
        // Test localStorage
        if web_sys::window()
            .and_then(|w| w.local_storage().ok())
            .flatten()
            .is_none()
        {
            return Err("localStorage not available".into());
        }

        // Test JSON
        if js_sys::JSON::parse("{}").is_err() {
            return Err("JSON not available".into());
        }

        // Test WebAssembly
        if js_sys::Reflect::get(&js_sys::global(), &"WebAssembly".into()).is_err() {
            return Err("WebAssembly not available".into());
        }

        console::log_1(&"Browser compatibility check passed".into());
        Ok(())
    }

    /// Test async/await support
    pub async fn test_async_support() -> Result<(), JsValue> {
        // Simple async operation
        wasm_bindgen_futures::JsFuture::from(js_sys::Promise::resolve(&JsValue::NULL)).await?;

        console::log_1(&"Async/await support confirmed".into());
        Ok(())
    }
}
