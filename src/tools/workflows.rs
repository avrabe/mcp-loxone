//! Workflow tools for tool composability
//!
//! This module provides tools for creating and executing workflows that chain
//! multiple tools together for complex automation scenarios.
//! For read-only workflow data, use resources:
//! - loxone://workflows/predefined - Predefined workflows
//! - loxone://workflows/examples - Workflow examples

use crate::error::{LoxoneError, Result};
use crate::server::workflow_engine::{Workflow, WorkflowCondition, WorkflowEngine, WorkflowStep};
use crate::tools::ToolContext;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing::info;

/// Workflow creation parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateWorkflowParams {
    pub name: String,
    pub description: String,
    pub steps: Vec<WorkflowStepDefinition>,
    pub timeout_seconds: Option<u64>,
    pub variables: Option<HashMap<String, serde_json::Value>>,
}

/// Workflow step definition for API
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowStepDefinition {
    #[serde(rename = "tool")]
    Tool {
        name: String,
        params: serde_json::Value,
        timeout_seconds: Option<u64>,
    },
    #[serde(rename = "parallel")]
    Parallel {
        steps: Vec<WorkflowStepDefinition>,
        timeout_seconds: Option<u64>,
    },
    #[serde(rename = "sequential")]
    Sequential { steps: Vec<WorkflowStepDefinition> },
    #[serde(rename = "conditional")]
    Conditional {
        condition: WorkflowConditionDefinition,
        if_true: Box<WorkflowStepDefinition>,
        if_false: Option<Box<WorkflowStepDefinition>>,
    },
    #[serde(rename = "delay")]
    Delay { duration_seconds: f64 },
    #[serde(rename = "loop")]
    Loop {
        step: Box<WorkflowStepDefinition>,
        count: usize,
        break_on_error: Option<bool>,
    },
}

/// Workflow condition definition for API
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowConditionDefinition {
    #[serde(rename = "success")]
    Success,
    #[serde(rename = "failed")]
    Failed,
    #[serde(rename = "contains")]
    Contains { value: String },
    #[serde(rename = "equals")]
    Equals { value: serde_json::Value },
    #[serde(rename = "expression")]
    Expression { code: String },
}

/// Execute workflow parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct ExecuteWorkflowParams {
    pub workflow_name: String,
    pub variables: Option<HashMap<String, serde_json::Value>>,
}

/// Workflow execution result
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkflowExecutionResult {
    pub workflow_id: String,
    pub execution_id: String,
    pub success: bool,
    pub result: Option<serde_json::Value>,
    pub duration_ms: u64,
    pub step_count: usize,
    pub error: Option<String>,
}

/// Predefined workflow parameters
#[derive(Debug, Serialize, Deserialize)]
pub struct ListPredefinedWorkflowsParams {}

/// Tool response type alias
pub type ToolResponse = Result<serde_json::Value>;

/// Convert API step definition to internal workflow step
fn convert_step(step_def: &WorkflowStepDefinition) -> WorkflowStep {
    match step_def {
        WorkflowStepDefinition::Tool {
            name,
            params,
            timeout_seconds,
        } => WorkflowStep::Tool {
            name: name.clone(),
            params: params.clone(),
            timeout: timeout_seconds.map(Duration::from_secs),
        },
        WorkflowStepDefinition::Parallel {
            steps,
            timeout_seconds,
        } => WorkflowStep::Parallel {
            steps: steps.iter().map(convert_step).collect(),
            timeout: timeout_seconds.map(Duration::from_secs),
        },
        WorkflowStepDefinition::Sequential { steps } => WorkflowStep::Sequential {
            steps: steps.iter().map(convert_step).collect(),
        },
        WorkflowStepDefinition::Conditional {
            condition,
            if_true,
            if_false,
        } => WorkflowStep::Conditional {
            condition: convert_condition(condition),
            if_true: Box::new(convert_step(if_true)),
            if_false: if_false.as_ref().map(|step| Box::new(convert_step(step))),
        },
        WorkflowStepDefinition::Delay { duration_seconds } => WorkflowStep::Delay {
            duration: Duration::from_secs_f64(*duration_seconds),
        },
        WorkflowStepDefinition::Loop {
            step,
            count,
            break_on_error,
        } => WorkflowStep::Loop {
            step: Box::new(convert_step(step)),
            count: *count,
            break_on_error: break_on_error.unwrap_or(true),
        },
    }
}

/// Convert API condition definition to internal workflow condition
fn convert_condition(condition_def: &WorkflowConditionDefinition) -> WorkflowCondition {
    match condition_def {
        WorkflowConditionDefinition::Success => WorkflowCondition::Success,
        WorkflowConditionDefinition::Failed => WorkflowCondition::Failed,
        WorkflowConditionDefinition::Contains { value } => WorkflowCondition::Contains {
            value: value.clone(),
        },
        WorkflowConditionDefinition::Equals { value } => WorkflowCondition::Equals {
            value: value.clone(),
        },
        WorkflowConditionDefinition::Expression { code } => {
            WorkflowCondition::Expression { code: code.clone() }
        }
    }
}

/// Create a new workflow
pub async fn create_workflow(_context: ToolContext, params: CreateWorkflowParams) -> ToolResponse {
    info!("Creating workflow: {}", params.name);

    let workflow_id = uuid::Uuid::new_v4().to_string();
    let root_step = if params.steps.len() == 1 {
        convert_step(&params.steps[0])
    } else {
        WorkflowStep::Sequential {
            steps: params.steps.iter().map(convert_step).collect(),
        }
    };

    let workflow = Workflow {
        id: workflow_id.clone(),
        name: params.name.clone(),
        description: params.description.clone(),
        root_step,
        timeout: params.timeout_seconds.map(Duration::from_secs),
        variables: params.variables.unwrap_or_default(),
    };

    // In a real implementation, this would be stored in a database or file
    // For now, we'll just return the workflow definition
    Ok(serde_json::json!({
        "workflow_id": workflow_id,
        "name": params.name,
        "description": params.description,
        "step_count": count_steps(&workflow.root_step),
        "created": true,
        "message": "Workflow created successfully. Note: This is a demo implementation - workflows are not persisted."
    }))
}

/// Execute a workflow (simplified demo version)
pub async fn execute_workflow_demo(
    context: ToolContext,
    params: ExecuteWorkflowParams,
) -> ToolResponse {
    info!("Executing demo workflow: {}", params.workflow_name);

    // Create a simple demo workflow based on the name
    let workflow = create_demo_workflow(&params.workflow_name, params.variables)?;

    // Create a tool executor that can call other MCP tools
    let _client = context.client.clone();
    let _server_context = context.context.clone();

    let tool_executor = move |tool_name: &str,
                              tool_params: serde_json::Value|
          -> Result<serde_json::Value> {
        // This is a simplified executor for demonstration
        // In a real implementation, this would call the actual MCP tools
        match tool_name {
            "list_rooms" => Ok(serde_json::json!({"rooms": ["Living Room", "Kitchen", "Bedroom"]})),
            "control_device" => {
                let device = tool_params
                    .get("device")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let action = tool_params
                    .get("action")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                Ok(serde_json::json!({
                    "device": device,
                    "action": action,
                    "status": "executed",
                    "success": true
                }))
            }
            "get_room_temperature" => {
                let room = tool_params
                    .get("room")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Living Room");
                Ok(serde_json::json!({
                    "room": room,
                    "temperature": 22.5,
                    "unit": "celsius"
                }))
            }
            _ => Err(LoxoneError::not_found(format!(
                "Demo tool not found: {tool_name}"
            ))),
        }
    };

    let engine = WorkflowEngine::new(tool_executor);
    let result = engine.execute_workflow(workflow).await;

    Ok(serde_json::json!({
        "workflow_id": result.workflow_id,
        "execution_id": result.execution_id,
        "success": result.success,
        "result": result.result,
        "duration_ms": result.duration.as_millis(),
        "step_count": result.step_results.len(),
        "error": result.error,
        "step_results": result.step_results
    }))
}

/// List predefined workflow templates
// READ-ONLY TOOL REMOVED:
// list_predefined_workflows() → Use resource: loxone://workflows/predefined
#[allow(dead_code)]
async fn _removed_list_predefined_workflows(
    _context: ToolContext,
    _params: ListPredefinedWorkflowsParams,
) -> ToolResponse {
    info!("Listing predefined workflow templates");

    let workflows = vec![
        serde_json::json!({
            "name": "morning_routine",
            "description": "Turn on lights and check temperature in all rooms",
            "steps": ["list_rooms", "turn_on_lights", "check_temperatures"],
            "estimated_duration": "30 seconds"
        }),
        serde_json::json!({
            "name": "security_check",
            "description": "Check all door and window sensors",
            "steps": ["get_all_door_window_sensors", "check_security_status"],
            "estimated_duration": "15 seconds"
        }),
        serde_json::json!({
            "name": "evening_routine",
            "description": "Dim lights and close rolladen in sequence",
            "steps": ["dim_all_lights", "close_all_rolladen"],
            "estimated_duration": "45 seconds"
        }),
        serde_json::json!({
            "name": "climate_optimization",
            "description": "Check and adjust temperature in all rooms",
            "steps": ["get_all_rooms", "check_temperatures", "adjust_heating"],
            "estimated_duration": "60 seconds"
        }),
        serde_json::json!({
            "name": "parallel_demo",
            "description": "Demonstrate parallel execution of multiple tasks",
            "steps": ["parallel: [list_rooms, get_temperature, check_security]"],
            "estimated_duration": "20 seconds"
        }),
    ];

    Ok(serde_json::json!({
        "predefined_workflows": workflows,
        "total_count": workflows.len(),
        "note": "These are demo workflows. Use 'execute_workflow_demo' to run them."
    }))
}

/// Get workflow examples and documentation
// READ-ONLY TOOL REMOVED:
// get_workflow_examples() → Use resource: loxone://workflows/examples
#[allow(dead_code)]
async fn _removed_get_workflow_examples(_context: ToolContext) -> ToolResponse {
    info!("Getting workflow examples and documentation");

    let examples = serde_json::json!({
        "workflow_concepts": {
            "description": "Workflows allow chaining multiple tools together for complex automation",
            "step_types": [
                "tool: Execute a single MCP tool",
                "sequential: Execute steps one after another",
                "parallel: Execute steps simultaneously",
                "conditional: Execute based on previous results",
                "delay: Wait for a specified duration",
                "loop: Repeat a step multiple times"
            ]
        },
        "simple_example": {
            "name": "Basic Light Control",
            "description": "Turn on lights in living room, then check status",
            "steps": [
                {
                    "type": "tool",
                    "name": "control_device",
                    "params": {"device": "Living Room Light", "action": "on"}
                },
                {
                    "type": "delay",
                    "duration_seconds": 2
                },
                {
                    "type": "tool",
                    "name": "get_device_status",
                    "params": {"device": "Living Room Light"}
                }
            ]
        },
        "conditional_example": {
            "name": "Smart Climate Control",
            "description": "Check temperature and adjust heating if needed",
            "steps": [
                {
                    "type": "tool",
                    "name": "get_room_temperature",
                    "params": {"room": "Living Room"}
                },
                {
                    "type": "conditional",
                    "condition": {"type": "contains", "value": "temperature"},
                    "if_true": {
                        "type": "tool",
                        "name": "adjust_heating",
                        "params": {"room": "Living Room", "target": 22}
                    }
                }
            ]
        },
        "parallel_example": {
            "name": "Multi-Room Status Check",
            "description": "Check status of multiple rooms simultaneously",
            "steps": [
                {
                    "type": "parallel",
                    "steps": [
                        {
                            "type": "tool",
                            "name": "get_room_status",
                            "params": {"room": "Living Room"}
                        },
                        {
                            "type": "tool",
                            "name": "get_room_status",
                            "params": {"room": "Kitchen"}
                        },
                        {
                            "type": "tool",
                            "name": "get_room_status",
                            "params": {"room": "Bedroom"}
                        }
                    ]
                }
            ]
        },
        "loop_example": {
            "name": "Sequential Room Control",
            "description": "Turn on lights in multiple rooms with delays",
            "steps": [
                {
                    "type": "loop",
                    "count": 3,
                    "break_on_error": false,
                    "step": {
                        "type": "sequential",
                        "steps": [
                            {
                                "type": "tool",
                                "name": "control_device",
                                "params": {"device": "${room_light}", "action": "on"}
                            },
                            {
                                "type": "delay",
                                "duration_seconds": 1
                            }
                        ]
                    }
                }
            ]
        },
        "usage_tips": [
            "Use 'create_workflow' to define custom workflows",
            "Use 'execute_workflow_demo' to run predefined workflows",
            "Workflows support variable substitution with ${variable_name}",
            "Conditional steps can check previous step results",
            "Parallel steps execute simultaneously for better performance",
            "Timeouts can be set at workflow or step level"
        ]
    });

    Ok(examples)
}

/// Helper function to count steps in a workflow
fn count_steps(step: &WorkflowStep) -> usize {
    match step {
        WorkflowStep::Tool { .. } => 1,
        WorkflowStep::Delay { .. } => 1,
        WorkflowStep::Sequential { steps } | WorkflowStep::Parallel { steps, .. } => {
            1 + steps.iter().map(count_steps).sum::<usize>()
        }
        WorkflowStep::Conditional {
            if_true, if_false, ..
        } => 1 + count_steps(if_true) + if_false.as_ref().map(|s| count_steps(s)).unwrap_or(0),
        WorkflowStep::Loop { step, count, .. } => 1 + count_steps(step) * count,
    }
}

/// Create a demo workflow based on name
fn create_demo_workflow(
    name: &str,
    variables: Option<HashMap<String, serde_json::Value>>,
) -> Result<Workflow> {
    let workflow_id = uuid::Uuid::new_v4().to_string();

    let (description, root_step) = match name {
        "morning_routine" => (
            "Turn on lights and check temperature in all rooms".to_string(),
            WorkflowStep::Sequential {
                steps: vec![
                    WorkflowStep::Tool {
                        name: "list_rooms".to_string(),
                        params: serde_json::json!({}),
                        timeout: None,
                    },
                    WorkflowStep::Tool {
                        name: "control_device".to_string(),
                        params: serde_json::json!({"device": "All Lights", "action": "on"}),
                        timeout: None,
                    },
                    WorkflowStep::Tool {
                        name: "get_room_temperature".to_string(),
                        params: serde_json::json!({"room": "Living Room"}),
                        timeout: None,
                    },
                ],
            },
        ),
        "parallel_demo" => (
            "Demonstrate parallel execution of multiple tasks".to_string(),
            WorkflowStep::Parallel {
                steps: vec![
                    WorkflowStep::Tool {
                        name: "list_rooms".to_string(),
                        params: serde_json::json!({}),
                        timeout: None,
                    },
                    WorkflowStep::Tool {
                        name: "get_room_temperature".to_string(),
                        params: serde_json::json!({"room": "Kitchen"}),
                        timeout: None,
                    },
                    WorkflowStep::Tool {
                        name: "control_device".to_string(),
                        params: serde_json::json!({"device": "Security System", "action": "status"}),
                        timeout: None,
                    },
                ],
                timeout: Some(Duration::from_secs(30)),
            },
        ),
        "conditional_demo" => (
            "Demonstrate conditional workflow execution".to_string(),
            WorkflowStep::Sequential {
                steps: vec![
                    WorkflowStep::Tool {
                        name: "get_room_temperature".to_string(),
                        params: serde_json::json!({"room": "Living Room"}),
                        timeout: None,
                    },
                    WorkflowStep::Conditional {
                        condition: WorkflowCondition::Success,
                        if_true: Box::new(WorkflowStep::Tool {
                            name: "control_device".to_string(),
                            params: serde_json::json!({"device": "Heating", "action": "adjust"}),
                            timeout: None,
                        }),
                        if_false: Some(Box::new(WorkflowStep::Tool {
                            name: "control_device".to_string(),
                            params: serde_json::json!({"device": "Alert", "action": "temperature_sensor_error"}),
                            timeout: None,
                        })),
                    },
                ],
            },
        ),
        _ => {
            return Err(LoxoneError::not_found(format!(
                "Demo workflow not found: {name}"
            )));
        }
    };

    Ok(Workflow {
        id: workflow_id,
        name: name.to_string(),
        description,
        root_step,
        timeout: Some(Duration::from_secs(120)),
        variables: variables.unwrap_or_default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_demo_workflow() {
        let workflow = create_demo_workflow("morning_routine", None);
        assert!(workflow.is_ok());

        let workflow = workflow.unwrap();
        assert_eq!(workflow.name, "morning_routine");
        assert!(workflow.description.contains("Turn on lights"));
    }

    #[tokio::test]
    async fn test_step_counting() {
        let simple_step = WorkflowStep::Tool {
            name: "test".to_string(),
            params: serde_json::json!({}),
            timeout: None,
        };
        assert_eq!(count_steps(&simple_step), 1);

        let sequential_step = WorkflowStep::Sequential {
            steps: vec![simple_step.clone(), simple_step.clone()],
        };
        assert_eq!(count_steps(&sequential_step), 3); // 1 container + 2 tools
    }
}
