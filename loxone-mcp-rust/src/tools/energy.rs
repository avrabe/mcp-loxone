//! Energy monitoring and management tools
//!
//! READ-ONLY TOOLS REMOVED:
//! The following tools were removed as they duplicate existing resources:
//!
//! - get_energy_consumption() → Use resource: loxone://energy/consumption
//! - get_power_meters() → Use resource: loxone://energy/meters
//! - get_solar_production() → Use resource: loxone://energy/solar
//!
//! These functions provided read-only data access and violated MCP patterns.
//! Use the corresponding resources for energy data retrieval instead.

use anyhow::Result;
// use rmcp::tool; // TODO: Re-enable when rmcp API is clarified
use serde_json::{json, Value};
use std::sync::Arc;

use crate::tools::ToolContext;

// READ-ONLY TOOL REMOVED:
// get_energy_consumption() → Use resource: loxone://energy/consumption
#[allow(dead_code)]
async fn _removed_get_energy_consumption(
    // #[description = "Get current energy consumption"] // TODO: Re-enable when rmcp API is clarified
    _input: Value,
    ctx: Arc<ToolContext>,
) -> Result<Value> {
    let _client = &ctx.client;

    // TODO: Implement get_status method in LoxoneClient
    let response = Value::Null; // Placeholder

    Ok(json!({
        "status": "success",
        "energy_data": response
    }))
}

// #[tool(name = "get_power_meters")] // TODO: Re-enable when rmcp API is clarified
// READ-ONLY TOOL REMOVED:
// get_power_meters() → Use resource: loxone://energy/meters
#[allow(dead_code)]
async fn _removed_get_power_meters(
    // #[description = "Get list of power meters"] // TODO: Re-enable when rmcp API is clarified
    _input: Value,
    ctx: Arc<ToolContext>,
) -> Result<Value> {
    let devices = ctx.context.devices.read().await;
    let meters: Vec<Value> = devices
        .values()
        .filter(|device| device.device_type == "PowerMeter")
        .map(|device| {
            json!({
                "uuid": device.uuid,
                "name": device.name,
                "room": device.room,
                "type": device.device_type
            })
        })
        .collect();

    Ok(json!({
        "status": "success",
        "power_meters": meters,
        "count": meters.len()
    }))
}

// #[tool(name = "get_solar_production")] // TODO: Re-enable when rmcp API is clarified
// READ-ONLY TOOL REMOVED:
// get_solar_production() → Use resource: loxone://energy/solar
#[allow(dead_code)]
async fn _removed_get_solar_production(
    // #[description = "Get solar panel production data"] // TODO: Re-enable when rmcp API is clarified
    _input: Value,
    ctx: Arc<ToolContext>,
) -> Result<Value> {
    let _client = &ctx.client;

    // TODO: Implement get_status method in LoxoneClient
    let response = Value::Null; // Placeholder

    Ok(json!({
        "status": "success",
        "solar_data": response
    }))
}

// #[tool(name = "optimize_energy_usage")] // TODO: Re-enable when rmcp API is clarified
pub async fn optimize_energy_usage(
    // #[description = "Trigger energy optimization routines"] // TODO: Re-enable when rmcp API is clarified
    _input: Value,
    ctx: Arc<ToolContext>,
) -> Result<Value> {
    let client = &ctx.client;

    client.send_command("energy/optimize", "optimize").await?;

    Ok(json!({
        "status": "success",
        "message": "Energy optimization initiated"
    }))
}
