//! Advanced Energy Management and Smart Grid Integration
//!
//! Comprehensive energy management including:
//! - Smart grid integration and demand response
//! - Solar production optimization and battery management
//! - Load balancing and peak shaving
//! - Energy consumption prediction and optimization
//! - Dynamic pricing integration
//! - Electric vehicle charging management
//! - Energy storage system control
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

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{info, warn};

use crate::client::LoxoneDevice;
use crate::tools::ToolContext;

/// Energy device types supported by the system
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EnergyDeviceType {
    /// Smart power meter
    SmartMeter,
    /// Solar panel system
    SolarPanels,
    /// Battery storage system
    BatteryStorage,
    /// Electric vehicle charger
    EVCharger,
    /// Heat pump
    HeatPump,
    /// Smart appliance
    SmartAppliance,
    /// Grid connection point
    GridConnection,
    /// Energy monitor
    EnergyMonitor,
    /// Unknown device
    Unknown(String),
}

/// Energy optimization mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum OptimizationMode {
    /// Minimize cost based on dynamic pricing
    CostOptimization,
    /// Maximize self-consumption of solar energy
    SelfConsumption,
    /// Balance grid load (peak shaving)
    LoadBalancing,
    /// Maximize green energy usage
    GreenEnergy,
    /// Emergency backup mode
    BackupPower,
    /// Custom optimization profile
    Custom(String),
}

/// Smart grid demand response event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemandResponseEvent {
    /// Event ID
    pub event_id: String,
    /// Event type (load_reduction, peak_event, etc.)
    pub event_type: String,
    /// Start time
    pub start_time: DateTime<Utc>,
    /// End time
    pub end_time: DateTime<Utc>,
    /// Target reduction in watts
    pub target_reduction_watts: Option<f64>,
    /// Incentive rate per kWh
    pub incentive_rate: Option<f64>,
    /// Priority level (1-10)
    pub priority: u8,
}

/// Energy storage system status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyStorageStatus {
    /// Storage system ID
    pub storage_id: String,
    /// Current charge level (0.0-1.0)
    pub charge_level: f64,
    /// Available capacity in kWh
    pub available_capacity_kwh: f64,
    /// Total capacity in kWh
    pub total_capacity_kwh: f64,
    /// Current power flow (positive = charging, negative = discharging)
    pub power_flow_kw: f64,
    /// Battery health (0.0-1.0)
    pub battery_health: f64,
    /// Operating mode
    pub mode: StorageMode,
    /// Temperature in Celsius
    pub temperature_c: Option<f64>,
}

/// Storage system operating mode
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum StorageMode {
    /// Charging from grid/solar
    Charging,
    /// Discharging to loads
    Discharging,
    /// Standby mode
    Standby,
    /// Backup power mode
    Backup,
    /// Grid support mode
    GridSupport,
    /// Maintenance mode
    Maintenance,
}

/// EV charger status and configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EVChargerStatus {
    /// Charger ID
    pub charger_id: String,
    /// Connected vehicle (if any)
    pub connected_vehicle: Option<String>,
    /// Current charging rate in kW
    pub charging_rate_kw: f64,
    /// Maximum charging rate in kW
    pub max_rate_kw: f64,
    /// Vehicle battery level (0.0-1.0)
    pub vehicle_battery_level: Option<f64>,
    /// Scheduled charging start
    pub scheduled_start: Option<DateTime<Utc>>,
    /// Target charge level
    pub target_charge_level: Option<f64>,
    /// Smart charging enabled
    pub smart_charging_enabled: bool,
}

/// Energy pricing information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnergyPricing {
    /// Current price per kWh
    pub current_price: f64,
    /// Currency code
    pub currency: String,
    /// Price tier (peak, off-peak, etc.)
    pub price_tier: String,
    /// Next price change time
    pub next_change: Option<DateTime<Utc>>,
    /// Price forecast for next 24 hours
    pub price_forecast: Vec<PriceForecast>,
}

/// Price forecast entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceForecast {
    /// Time period start
    pub time: DateTime<Utc>,
    /// Price per kWh
    pub price: f64,
    /// Price tier
    pub tier: String,
}

/// Control battery storage systems
pub async fn control_battery_storage(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;

    #[derive(Deserialize)]
    struct BatteryRequest {
        #[serde(default)]
        battery_id: Option<String>,
        action: String, // "charge", "discharge", "standby", "backup"
        #[serde(default)]
        power_kw: Option<f64>,
        #[serde(default)]
        target_soc: Option<f64>, // State of charge 0-100
        #[serde(default)]
        mode: Option<String>,
    }

    let request: BatteryRequest =
        serde_json::from_value(input).map_err(|e| anyhow!("Invalid battery request: {}", e))?;

    let battery_id = request
        .battery_id
        .unwrap_or_else(|| "battery/main".to_string());

    info!(
        "Controlling battery storage {}: {}",
        battery_id, request.action
    );

    let command = match request.action.as_str() {
        "charge" => {
            let power = request.power_kw.unwrap_or(3.0);
            format!("charge/{}", (power * 1000.0) as i32)
        }
        "discharge" => {
            let power = request.power_kw.unwrap_or(3.0);
            format!("discharge/{}", (power * 1000.0) as i32)
        }
        "standby" => "standby".to_string(),
        "backup" => "backup_mode".to_string(),
        _ => {
            return Err(anyhow!(
                "Invalid action: {}. Use 'charge', 'discharge', 'standby', or 'backup'",
                request.action
            ))
        }
    };

    client
        .send_command(&battery_id, &command)
        .await
        .map_err(|e| anyhow!("Failed to control battery: {}", e))?;

    // Set target SOC if specified
    if let Some(target_soc) = request.target_soc {
        let soc_command = format!("target_soc/{}", target_soc.clamp(0.0, 100.0) as i32);
        if let Err(e) = client.send_command(&battery_id, &soc_command).await {
            warn!("Failed to set target SOC: {}", e);
        }
    }

    // Set mode if specified
    if let Some(mode) = &request.mode {
        let mode_command = format!("mode/{}", mode);
        if let Err(e) = client.send_command(&battery_id, &mode_command).await {
            warn!("Failed to set battery mode: {}", e);
        }
    }

    Ok(json!({
        "status": "success",
        "battery_id": battery_id,
        "action": request.action,
        "power_kw": request.power_kw,
        "target_soc": request.target_soc,
        "mode": request.mode,
        "timestamp": Utc::now()
    }))
}

/// Manage EV charging with smart grid integration
pub async fn manage_ev_charging(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;

    #[derive(Deserialize)]
    struct EVChargingRequest {
        charger_id: String,
        action: String, // "start", "stop", "schedule", "smart_charge"
        #[serde(default)]
        charging_rate_kw: Option<f64>,
        #[serde(default)]
        target_soc: Option<f64>,
        #[serde(default)]
        departure_time: Option<DateTime<Utc>>,
        #[serde(default)]
        use_solar_only: Option<bool>,
        #[serde(default)]
        price_limit: Option<f64>,
    }

    let request: EVChargingRequest =
        serde_json::from_value(input).map_err(|e| anyhow!("Invalid EV charging request: {}", e))?;

    info!(
        "Managing EV charger {}: {}",
        request.charger_id, request.action
    );

    match request.action.as_str() {
        "start" => {
            let rate = request.charging_rate_kw.unwrap_or(11.0).clamp(1.4, 22.0);
            let command = format!("charge/start/{}", (rate * 1000.0) as i32);
            client.send_command(&request.charger_id, &command).await?;

            Ok(json!({
                "status": "success",
                "message": "EV charging started",
                "charger_id": request.charger_id,
                "charging_rate_kw": rate
            }))
        }
        "stop" => {
            client
                .send_command(&request.charger_id, "charge/stop")
                .await?;

            Ok(json!({
                "status": "success",
                "message": "EV charging stopped",
                "charger_id": request.charger_id
            }))
        }
        "schedule" => {
            let departure = request
                .departure_time
                .ok_or_else(|| anyhow!("Departure time required for scheduled charging"))?;

            // Calculate optimal charging schedule
            let hours_until_departure = (departure - Utc::now()).num_hours().max(0) as u32;
            let target_soc = request.target_soc.unwrap_or(80.0);

            let command = format!("schedule/{}/{}", hours_until_departure, target_soc as i32);
            client.send_command(&request.charger_id, &command).await?;

            Ok(json!({
                "status": "success",
                "message": "EV charging scheduled",
                "charger_id": request.charger_id,
                "departure_time": departure,
                "target_soc": target_soc
            }))
        }
        "smart_charge" => {
            // Enable smart charging based on grid conditions and pricing
            let mut config = vec!["smart_mode/enable"];

            if let Some(true) = request.use_solar_only {
                config.push("solar_only/true");
            }

            if let Some(price_limit) = request.price_limit {
                let limit_command = format!("price_limit/{}", (price_limit * 100.0) as i32);
                client
                    .send_command(&request.charger_id, &limit_command)
                    .await?;
            }

            for cmd in config {
                if let Err(e) = client.send_command(&request.charger_id, cmd).await {
                    warn!("Failed to apply smart charging config {}: {}", cmd, e);
                }
            }

            Ok(json!({
                "status": "success",
                "message": "Smart charging enabled",
                "charger_id": request.charger_id,
                "use_solar_only": request.use_solar_only,
                "price_limit": request.price_limit
            }))
        }
        _ => Err(anyhow!(
            "Invalid action: {}. Use 'start', 'stop', 'schedule', or 'smart_charge'",
            request.action
        )),
    }
}

/// Respond to smart grid demand response events
pub async fn handle_demand_response(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;

    #[derive(Deserialize)]
    struct DemandResponseRequest {
        event_type: String, // "reduce_load", "shift_load", "increase_load"
        #[serde(default)]
        target_reduction_kw: Option<f64>,
        #[serde(default)]
        duration_minutes: Option<u32>,
        #[serde(default)]
        priority_level: Option<u8>,
        #[serde(default)]
        exclude_critical: Option<bool>,
    }

    let request: DemandResponseRequest = serde_json::from_value(input)
        .map_err(|e| anyhow!("Invalid demand response request: {}", e))?;

    warn!(
        "DEMAND RESPONSE EVENT: {} - Target: {:?}kW",
        request.event_type, request.target_reduction_kw
    );

    let mut actions_taken = Vec::new();
    let mut reduction_achieved_kw = 0.0;

    match request.event_type.as_str() {
        "reduce_load" => {
            let target = request.target_reduction_kw.unwrap_or(5.0);
            let priority = request.priority_level.unwrap_or(4); // Default to all priorities

            // Priority 1: Reduce EV charging
            if priority >= 1 {
                if let Ok(_) = client.send_command("ev/all", "reduce_power/50").await {
                    actions_taken.push("Reduced EV charging power by 50%".to_string());
                    reduction_achieved_kw += 5.5; // Assuming 11kW charger reduced to 5.5kW
                }
            }

            // Priority 2: Defer water heating
            if priority >= 2 && reduction_achieved_kw < target {
                if let Ok(_) = client.send_command("water_heater", "defer").await {
                    actions_taken.push("Deferred water heating".to_string());
                    reduction_achieved_kw += 3.0;
                }
            }

            // Priority 3: Reduce HVAC
            if priority >= 3
                && reduction_achieved_kw < target
                && request.exclude_critical != Some(true)
            {
                if let Ok(_) = client.send_command("hvac/all", "eco_mode").await {
                    actions_taken.push("Set HVAC to eco mode".to_string());
                    reduction_achieved_kw += 2.0;
                }
            }

            // Priority 4: Dim non-essential lighting
            if priority >= 4 && reduction_achieved_kw < target {
                if let Ok(_) = client.send_command("lights/non_essential", "dim/50").await {
                    actions_taken.push("Dimmed non-essential lighting by 50%".to_string());
                    reduction_achieved_kw += 0.5;
                }
            }
        }
        "shift_load" => {
            // Shift flexible loads to later time
            let delay_hours = (request.duration_minutes.unwrap_or(120) / 60) as i32;

            if let Ok(_) = client
                .send_command("scheduler/flexible", &format!("delay/{}", delay_hours))
                .await
            {
                actions_taken.push(format!("Shifted flexible loads by {} hours", delay_hours));
            }

            // Pause non-critical operations
            if let Ok(_) = client
                .send_command("operations/non_critical", "pause")
                .await
            {
                actions_taken.push("Paused non-critical operations".to_string());
            }
        }
        "increase_load" => {
            // Rare case: grid has excess renewable energy

            // Charge batteries
            if let Ok(_) = client.send_command("battery/all", "charge/max").await {
                actions_taken.push("Started maximum battery charging".to_string());
            }

            // Start EV charging
            if let Ok(_) = client.send_command("ev/all", "charge/max").await {
                actions_taken.push("Started maximum EV charging".to_string());
            }

            // Pre-heat water
            if let Ok(_) = client.send_command("water_heater", "boost").await {
                actions_taken.push("Started water heater boost mode".to_string());
            }
        }
        _ => return Err(anyhow!("Invalid event type: {}", request.event_type)),
    }

    Ok(json!({
        "status": "success",
        "event_type": request.event_type,
        "actions_taken": actions_taken,
        "reduction_achieved_kw": reduction_achieved_kw,
        "duration_minutes": request.duration_minutes,
        "priority_level": request.priority_level,
        "exclude_critical": request.exclude_critical,
        "timestamp": Utc::now()
    }))
}

/// Set dynamic energy pricing for optimization
pub async fn set_energy_pricing(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;

    #[derive(Deserialize)]
    struct PricingRequest {
        #[serde(default)]
        current_price: Option<f64>,
        #[serde(default)]
        currency: Option<String>,
        #[serde(default)]
        price_forecast: Option<Vec<PriceForecast>>,
        #[serde(default)]
        update_optimization: Option<bool>,
    }

    let request: PricingRequest =
        serde_json::from_value(input).map_err(|e| anyhow!("Invalid pricing request: {}", e))?;

    let current_price = request.current_price.unwrap_or(0.25);
    let currency = request.currency.unwrap_or_else(|| "EUR".to_string());

    info!("Setting energy price: {} {}/kWh", current_price, currency);

    // Update current price
    let price_cents = (current_price * 100.0) as i32;
    client
        .send_command("energy/pricing", &format!("current/{}", price_cents))
        .await?;

    // Update price forecast if provided
    let forecast_updated = if let Some(ref forecast) = request.price_forecast {
        for (i, price_point) in forecast.iter().take(24).enumerate() {
            let forecast_cmd = format!("forecast/{}/{}", i, (price_point.price * 100.0) as i32);
            if let Err(e) = client.send_command("energy/pricing", &forecast_cmd).await {
                warn!("Failed to set forecast price point {}: {}", i, e);
            }
        }
        true
    } else {
        false
    };

    // Update optimization strategy based on pricing
    if request.update_optimization.unwrap_or(true) {
        let price_tier = if current_price < 0.15 {
            "low"
        } else if current_price < 0.30 {
            "normal"
        } else {
            "high"
        };

        client
            .send_command("energy/optimize", &format!("price_tier/{}", price_tier))
            .await?;
    }

    Ok(json!({
        "status": "success",
        "current_price": current_price,
        "currency": currency,
        "forecast_updated": forecast_updated,
        "optimization_updated": request.update_optimization.unwrap_or(true),
        "timestamp": Utc::now()
    }))
}

/// Configure load priority for energy management
pub async fn configure_load_priority(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;

    #[derive(Deserialize)]
    struct LoadPriorityRequest {
        priorities: Vec<LoadPriority>,
        #[serde(default)]
        apply_immediately: Option<bool>,
    }

    #[derive(Deserialize)]
    struct LoadPriority {
        device_id: String,
        priority: u8, // 1-10, 1 being highest
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        can_defer: Option<bool>,
        #[serde(default)]
        can_interrupt: Option<bool>,
    }

    let request: LoadPriorityRequest = serde_json::from_value(input)
        .map_err(|e| anyhow!("Invalid load priority request: {}", e))?;

    info!(
        "Configuring load priorities for {} devices",
        request.priorities.len()
    );

    let mut configured = Vec::new();

    for load in request.priorities {
        let priority_cmd = format!("priority/{}", load.priority.clamp(1, 10));

        match client.send_command(&load.device_id, &priority_cmd).await {
            Ok(_) => {
                configured.push(json!({
                    "device_id": load.device_id,
                    "priority": load.priority,
                    "category": load.category.clone(),
                    "status": "configured"
                }));

                // Set category if provided
                if let Some(ref category) = load.category {
                    let _ = client
                        .send_command(&load.device_id, &format!("category/{}", category))
                        .await;
                }

                // Set additional flags
                if let Some(true) = load.can_defer {
                    let _ = client
                        .send_command(&load.device_id, "deferrable/true")
                        .await;
                }

                if let Some(true) = load.can_interrupt {
                    let _ = client
                        .send_command(&load.device_id, "interruptible/true")
                        .await;
                }
            }
            Err(e) => {
                configured.push(json!({
                    "device_id": load.device_id,
                    "priority": load.priority,
                    "category": load.category.clone(),
                    "status": "failed",
                    "error": e.to_string()
                }));
            }
        }
    }

    // Apply new priority configuration
    if request.apply_immediately.unwrap_or(true) {
        client.send_command("energy/priorities", "apply").await?;
    }

    Ok(json!({
        "status": "success",
        "configured_devices": configured,
        "applied": request.apply_immediately.unwrap_or(true),
        "timestamp": Utc::now()
    }))
}

/// Get comprehensive energy system status
pub async fn get_energy_system_status(_input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;
    let devices = ctx.context.devices.read().await;

    // Find all energy-related devices
    let energy_devices: Vec<&LoxoneDevice> = devices
        .values()
        .filter(|device| is_energy_device(&device.device_type))
        .collect();

    let mut meters = Vec::new();
    let mut solar_systems = Vec::new();
    let mut storage_systems = Vec::new();
    let mut ev_chargers = Vec::new();

    // Categorize devices and get their states
    let device_uuids: Vec<String> = energy_devices.iter().map(|d| d.uuid.clone()).collect();
    let device_states = client.get_device_states(&device_uuids).await?;

    for device in &energy_devices {
        if let Some(state) = device_states.get(&device.uuid) {
            match classify_energy_device(&device.device_type) {
                EnergyDeviceType::SmartMeter | EnergyDeviceType::EnergyMonitor => {
                    meters.push(json!({
                        "uuid": device.uuid,
                        "name": device.name,
                        "type": device.device_type,
                        "room": device.room,
                        "current_power_w": state.get("power").and_then(|v| v.as_f64()),
                        "total_energy_kwh": state.get("energy").and_then(|v| v.as_f64()),
                    }));
                }
                EnergyDeviceType::SolarPanels => {
                    solar_systems.push(json!({
                        "uuid": device.uuid,
                        "name": device.name,
                        "current_production_w": state.get("power").and_then(|v| v.as_f64()),
                        "daily_production_kwh": state.get("daily_energy").and_then(|v| v.as_f64()),
                    }));
                }
                EnergyDeviceType::BatteryStorage => {
                    let charge_level =
                        state.get("soc").and_then(|v| v.as_f64()).unwrap_or(0.0) / 100.0;
                    storage_systems.push(EnergyStorageStatus {
                        storage_id: device.uuid.clone(),
                        charge_level,
                        available_capacity_kwh: charge_level * 10.0, // Assuming 10kWh battery
                        total_capacity_kwh: 10.0,
                        power_flow_kw: state.get("power").and_then(|v| v.as_f64()).unwrap_or(0.0)
                            / 1000.0,
                        battery_health: 0.95,
                        mode: StorageMode::Standby,
                        temperature_c: state.get("temperature").and_then(|v| v.as_f64()),
                    });
                }
                EnergyDeviceType::EVCharger => {
                    ev_chargers.push(EVChargerStatus {
                        charger_id: device.uuid.clone(),
                        connected_vehicle: state
                            .get("vehicle_id")
                            .and_then(|v| v.as_str())
                            .map(String::from),
                        charging_rate_kw: state
                            .get("power")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0)
                            / 1000.0,
                        max_rate_kw: 11.0, // Standard 11kW charger
                        vehicle_battery_level: state
                            .get("vehicle_soc")
                            .and_then(|v| v.as_f64())
                            .map(|v| v / 100.0),
                        scheduled_start: None,
                        target_charge_level: Some(0.8),
                        smart_charging_enabled: true,
                    });
                }
                _ => {}
            }
        }
    }

    Ok(json!({
        "status": "success",
        "energy_system": {
            "meters": meters,
            "solar_systems": solar_systems,
            "storage_systems": storage_systems,
            "ev_chargers": ev_chargers,
            "device_count": energy_devices.len(),
        },
        "timestamp": Utc::now()
    }))
}

/// Check if device is energy-related
fn is_energy_device(device_type: &str) -> bool {
    let energy_keywords = [
        "meter",
        "power",
        "energy",
        "solar",
        "battery",
        "storage",
        "charger",
        "ev",
        "heat",
        "pump",
        "grid",
        "consumption",
    ];

    let device_lower = device_type.to_lowercase();
    energy_keywords
        .iter()
        .any(|keyword| device_lower.contains(keyword))
}

/// Classify energy device type
fn classify_energy_device(device_type: &str) -> EnergyDeviceType {
    let device_lower = device_type.to_lowercase();

    if device_lower.contains("meter") || device_lower.contains("monitor") {
        EnergyDeviceType::SmartMeter
    } else if device_lower.contains("solar") || device_lower.contains("pv") {
        EnergyDeviceType::SolarPanels
    } else if device_lower.contains("battery") || device_lower.contains("storage") {
        EnergyDeviceType::BatteryStorage
    } else if device_lower.contains("charger") || device_lower.contains("ev") {
        EnergyDeviceType::EVCharger
    } else if device_lower.contains("heat") && device_lower.contains("pump") {
        EnergyDeviceType::HeatPump
    } else if device_lower.contains("grid") {
        EnergyDeviceType::GridConnection
    } else {
        EnergyDeviceType::Unknown(device_type.to_string())
    }
}

/// Optimize energy usage with advanced strategies
pub async fn optimize_energy_usage(input: Value, ctx: Arc<ToolContext>) -> Result<Value> {
    let client = &ctx.client;

    #[derive(Deserialize)]
    struct OptimizationRequest {
        #[serde(default)]
        mode: Option<String>,
        #[serde(default)]
        duration_hours: Option<u32>,
        #[serde(default)]
        target_savings_percent: Option<f64>,
        #[serde(default)]
        allow_comfort_reduction: Option<bool>,
    }

    let request: OptimizationRequest = serde_json::from_value(input)
        .map_err(|e| anyhow!("Invalid optimization request: {}", e))?;

    let mode = match request.mode.as_deref() {
        Some("cost") => OptimizationMode::CostOptimization,
        Some("solar") => OptimizationMode::SelfConsumption,
        Some("load") => OptimizationMode::LoadBalancing,
        Some("green") => OptimizationMode::GreenEnergy,
        Some("backup") => OptimizationMode::BackupPower,
        _ => OptimizationMode::CostOptimization,
    };

    info!("Starting energy optimization with mode: {:?}", mode);

    let mut actions_taken = Vec::new();

    // Execute optimization based on mode
    match mode {
        OptimizationMode::CostOptimization => {
            // Shift loads to off-peak hours
            if let Err(e) = client.send_command("energy/schedule", "shift_loads").await {
                warn!("Failed to shift loads: {}", e);
            } else {
                actions_taken.push("Shifted flexible loads to off-peak hours".to_string());
            }

            // Reduce non-essential consumption during peak hours
            if let Err(e) = client.send_command("energy/reduce", "peak_reduction").await {
                warn!("Failed to reduce peak consumption: {}", e);
            } else {
                actions_taken
                    .push("Reduced non-essential consumption during peak pricing".to_string());
            }
        }
        OptimizationMode::SelfConsumption => {
            // Prioritize solar energy usage
            if let Err(e) = client
                .send_command("energy/solar", "maximize_self_use")
                .await
            {
                warn!("Failed to maximize solar self-consumption: {}", e);
            } else {
                actions_taken.push("Prioritized loads during solar production hours".to_string());
            }

            // Charge batteries from solar
            if let Err(e) = client.send_command("battery/charge", "solar_only").await {
                warn!("Failed to set solar-only charging: {}", e);
            } else {
                actions_taken.push("Configured battery to charge from solar only".to_string());
            }
        }
        OptimizationMode::LoadBalancing => {
            // Implement peak shaving
            if let Err(e) = client.send_command("energy/balance", "peak_shave").await {
                warn!("Failed to enable peak shaving: {}", e);
            } else {
                actions_taken.push("Enabled peak shaving to reduce grid stress".to_string());
            }
        }
        _ => {
            // Generic optimization
            client.send_command("energy/optimize", "auto").await?;
            actions_taken.push("Applied automatic energy optimization".to_string());
        }
    }

    Ok(json!({
        "status": "success",
        "optimization_mode": format!("{:?}", mode),
        "actions_taken": actions_taken,
        "duration_hours": request.duration_hours.unwrap_or(24),
        "target_savings": request.target_savings_percent,
        "timestamp": Utc::now()
    }))
}
