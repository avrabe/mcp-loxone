use anyhow::Result;
// use rmcp::tool; // TODO: Re-enable when rmcp API is clarified
use serde_json::{json, Value};
use std::sync::Arc;

use crate::tools::ToolContext;

// #[tool(name = "get_energy_consumption")] // TODO: Re-enable when rmcp API is clarified
pub async fn get_energy_consumption(
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
pub async fn get_power_meters(
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
pub async fn get_solar_production(
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
