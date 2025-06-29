//! Security system control tools
//!
//! Tools for security system control and alarm management.
//! For read-only security data, use resources:
//! - loxone://security/status - Security system status
//! - loxone://security/zones - Security zones

use anyhow::Result;
// use rmcp::tool; // TODO: Re-enable when rmcp API is clarified
use serde_json::{json, Value};
use std::sync::Arc;

use crate::tools::ToolContext;

// READ-ONLY TOOL REMOVED:
// get_alarm_status() → Use resource: loxone://security/status
#[allow(dead_code)]
async fn _removed_get_alarm_status(
    // #[description = "Get current alarm system status"] // TODO: Re-enable when rmcp API is clarified
    _input: Value,
    ctx: Arc<ToolContext>,
) -> Result<Value> {
    let _client = &ctx.client;

    // TODO: Implement get_status method in LoxoneClient
    let response = Value::Null; // Placeholder

    Ok(json!({
        "status": "success",
        "alarm_status": response
    }))
}

// #[tool(name = "arm_alarm")] // TODO: Re-enable when rmcp API is clarified
pub async fn arm_alarm(
    // #[description = "Arm the alarm system"] // TODO: Re-enable when rmcp API is clarified
    _input: Value,
    ctx: Arc<ToolContext>,
) -> Result<Value> {
    let client = &ctx.client;

    client.send_command("alarm/arm", "arm").await?;

    Ok(json!({
        "status": "success",
        "message": "Alarm system armed"
    }))
}

// #[tool(name = "disarm_alarm")] // TODO: Re-enable when rmcp API is clarified
pub async fn disarm_alarm(
    // #[description = "Disarm the alarm system"] // TODO: Re-enable when rmcp API is clarified
    _input: Value,
    ctx: Arc<ToolContext>,
) -> Result<Value> {
    let client = &ctx.client;

    client.send_command("alarm/disarm", "disarm").await?;

    Ok(json!({
        "status": "success",
        "message": "Alarm system disarmed"
    }))
}

// #[tool(name = "get_security_cameras")] // TODO: Re-enable when rmcp API is clarified
// READ-ONLY TOOL REMOVED:
// get_security_cameras() → Use resource: loxone://security/cameras
#[allow(dead_code)]
async fn _removed_get_security_cameras(
    // #[description = "Get list of security cameras"] // TODO: Re-enable when rmcp API is clarified
    _input: Value,
    ctx: Arc<ToolContext>,
) -> Result<Value> {
    let devices = ctx.context.devices.read().await;
    let cameras: Vec<Value> = devices
        .values()
        .filter(|device| device.device_type == "Camera")
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
        "cameras": cameras,
        "count": cameras.len()
    }))
}
