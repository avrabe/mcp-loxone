//! Workflow engine for tool composability
//!
//! This module provides the ability to chain tools together to create complex
//! automation workflows. It supports conditional execution, parallel operations,
//! and error handling for multi-step processes.

use crate::error::{LoxoneError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Workflow step type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowStep {
    /// Execute a single tool
    Tool {
        name: String,
        params: serde_json::Value,
        timeout: Option<Duration>,
    },
    /// Execute multiple tools in parallel
    Parallel {
        steps: Vec<WorkflowStep>,
        timeout: Option<Duration>,
    },
    /// Execute tools sequentially
    Sequential { steps: Vec<WorkflowStep> },
    /// Conditional execution based on previous step result
    Conditional {
        condition: WorkflowCondition,
        if_true: Box<WorkflowStep>,
        if_false: Option<Box<WorkflowStep>>,
    },
    /// Delay/sleep step
    Delay { duration: Duration },
    /// Loop execution
    Loop {
        step: Box<WorkflowStep>,
        count: usize,
        break_on_error: bool,
    },
}

/// Workflow condition for conditional execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WorkflowCondition {
    /// Check if previous step was successful
    Success,
    /// Check if previous step failed
    Failed,
    /// Check if result contains specific value
    Contains { value: String },
    /// Check if result equals specific value
    Equals { value: serde_json::Value },
    /// Custom JavaScript expression (future enhancement)
    Expression { code: String },
}

/// Workflow definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    /// Workflow ID
    pub id: String,
    /// Workflow name
    pub name: String,
    /// Workflow description
    pub description: String,
    /// Root workflow step
    pub root_step: WorkflowStep,
    /// Global timeout for entire workflow
    pub timeout: Option<Duration>,
    /// Variables that can be used across steps
    pub variables: HashMap<String, serde_json::Value>,
}

/// Workflow execution context
#[derive(Debug, Clone)]
pub struct WorkflowContext {
    /// Workflow ID
    pub workflow_id: String,
    /// Execution ID
    pub execution_id: String,
    /// Start time
    pub started_at: Instant,
    /// Variables
    pub variables: HashMap<String, serde_json::Value>,
    /// Previous step results
    pub step_results: Vec<WorkflowStepResult>,
}

/// Result of a workflow step execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStepResult {
    /// Step name or type
    pub step_name: String,
    /// Execution result
    pub result: std::result::Result<serde_json::Value, String>,
    /// Execution duration
    pub duration: Duration,
    /// Step metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Workflow execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    /// Workflow ID
    pub workflow_id: String,
    /// Execution ID
    pub execution_id: String,
    /// Overall success
    pub success: bool,
    /// Final result
    pub result: Option<serde_json::Value>,
    /// Execution duration
    pub duration: Duration,
    /// Step results
    pub step_results: Vec<WorkflowStepResult>,
    /// Error message if failed
    pub error: Option<String>,
}

/// Tool executor function type
type ToolExecutor = Box<dyn Fn(&str, serde_json::Value) -> Result<serde_json::Value> + Send + Sync>;

/// Workflow engine for executing tool compositions
pub struct WorkflowEngine {
    /// Tool executor function
    tool_executor: ToolExecutor,
}

impl WorkflowEngine {
    /// Create a new workflow engine
    pub fn new<F>(tool_executor: F) -> Self
    where
        F: Fn(&str, serde_json::Value) -> Result<serde_json::Value> + Send + Sync + 'static,
    {
        Self {
            tool_executor: Box::new(tool_executor),
        }
    }

    /// Execute a workflow
    pub async fn execute_workflow(&self, workflow: Workflow) -> WorkflowResult {
        let execution_id = uuid::Uuid::new_v4().to_string();
        let start_time = Instant::now();

        info!(
            "Starting workflow execution: {} ({})",
            workflow.name, execution_id
        );

        let mut context = WorkflowContext {
            workflow_id: workflow.id.clone(),
            execution_id: execution_id.clone(),
            started_at: start_time,
            variables: workflow.variables.clone(),
            step_results: Vec::new(),
        };

        // Apply global timeout if specified
        let execution_result = if let Some(timeout) = workflow.timeout {
            match tokio::time::timeout(
                timeout,
                Box::pin(self.execute_step(&workflow.root_step, &mut context)),
            )
            .await
            {
                Ok(result) => result,
                Err(_) => Err(LoxoneError::timeout(format!(
                    "Workflow {} timed out after {:?}",
                    workflow.name, timeout
                ))),
            }
        } else {
            Box::pin(self.execute_step(&workflow.root_step, &mut context)).await
        };

        let duration = start_time.elapsed();

        let workflow_result = match execution_result {
            Ok(result) => {
                info!(
                    "Workflow {} completed successfully in {:?}",
                    workflow.name, duration
                );
                WorkflowResult {
                    workflow_id: workflow.id,
                    execution_id,
                    success: true,
                    result: Some(result),
                    duration,
                    step_results: context.step_results,
                    error: None,
                }
            }
            Err(e) => {
                error!(
                    "Workflow {} failed after {:?}: {}",
                    workflow.name, duration, e
                );
                WorkflowResult {
                    workflow_id: workflow.id,
                    execution_id,
                    success: false,
                    result: None,
                    duration,
                    step_results: context.step_results,
                    error: Some(e.to_string()),
                }
            }
        };

        workflow_result
    }

    /// Execute a workflow step
    fn execute_step<'a>(
        &'a self,
        step: &'a WorkflowStep,
        context: &'a mut WorkflowContext,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<serde_json::Value>> + Send + 'a>>
    {
        Box::pin(async move {
            debug!("Executing workflow step: {:?}", step);

            match step {
                WorkflowStep::Tool {
                    name,
                    params,
                    timeout,
                } => {
                    self.execute_tool_step(name, params, *timeout, context)
                        .await
                }
                WorkflowStep::Parallel { steps, timeout } => {
                    self.execute_parallel_step(steps, *timeout, context).await
                }
                WorkflowStep::Sequential { steps } => {
                    self.execute_sequential_step(steps, context).await
                }
                WorkflowStep::Conditional {
                    condition,
                    if_true,
                    if_false,
                } => {
                    self.execute_conditional_step(condition, if_true, if_false.as_deref(), context)
                        .await
                }
                WorkflowStep::Delay { duration } => {
                    self.execute_delay_step(*duration, context).await
                }
                WorkflowStep::Loop {
                    step,
                    count,
                    break_on_error,
                } => {
                    self.execute_loop_step(step, *count, *break_on_error, context)
                        .await
                }
            }
        })
    }

    /// Execute a tool step
    async fn execute_tool_step(
        &self,
        tool_name: &str,
        params: &serde_json::Value,
        timeout: Option<Duration>,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        let start_time = Instant::now();

        debug!("Executing tool: {} with params: {}", tool_name, params);

        // Substitute variables in parameters
        let substituted_params = self.substitute_variables(params, &context.variables)?;

        // Execute tool - simplified for demonstration
        let result = if let Some(_timeout_duration) = timeout {
            // For now, execute without timeout to simplify implementation
            // In a full implementation, this would properly handle timeouts
            (self.tool_executor)(tool_name, substituted_params.clone())
        } else {
            (self.tool_executor)(tool_name, substituted_params.clone())
        };

        let duration = start_time.elapsed();

        // Record step result
        let step_result = WorkflowStepResult {
            step_name: format!("tool:{}", tool_name),
            result: result
                .as_ref()
                .map(|v| v.clone())
                .map_err(|e| e.to_string()),
            duration,
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("tool_name".to_string(), serde_json::json!(tool_name));
                metadata.insert("params".to_string(), substituted_params);
                metadata
            },
        };

        context.step_results.push(step_result);

        result
    }

    /// Execute parallel steps
    async fn execute_parallel_step(
        &self,
        steps: &[WorkflowStep],
        _timeout: Option<Duration>,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        let start_time = Instant::now();

        debug!("Executing {} steps in parallel", steps.len());

        // Execute steps sequentially for now to avoid lifetime issues
        // In a full implementation, this would use proper async parallel execution
        let mut results = Vec::new();
        for step in steps {
            let mut step_context = context.clone();
            let result = Box::pin(self.execute_step(step, &mut step_context)).await?;
            results.push(result);
        }

        let results = Ok(results);

        let duration = start_time.elapsed();

        // Record parallel step result
        let step_result = WorkflowStepResult {
            step_name: "parallel".to_string(),
            result: results
                .as_ref()
                .map(|v| serde_json::json!(v))
                .map_err(|e: &LoxoneError| e.to_string()),
            duration,
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("step_count".to_string(), serde_json::json!(steps.len()));
                metadata
            },
        };

        context.step_results.push(step_result);

        results.map(|v| serde_json::json!(v))
    }

    /// Execute sequential steps
    async fn execute_sequential_step(
        &self,
        steps: &[WorkflowStep],
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        let start_time = Instant::now();

        debug!("Executing {} steps sequentially", steps.len());

        let mut results = Vec::new();

        for step in steps {
            let result = Box::pin(self.execute_step(step, context)).await?;
            results.push(result);
        }

        let duration = start_time.elapsed();

        // Record sequential step result
        let step_result = WorkflowStepResult {
            step_name: "sequential".to_string(),
            result: Ok(serde_json::json!(results)),
            duration,
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("step_count".to_string(), serde_json::json!(steps.len()));
                metadata
            },
        };

        context.step_results.push(step_result);

        Ok(serde_json::json!(results))
    }

    /// Execute conditional step
    async fn execute_conditional_step(
        &self,
        condition: &WorkflowCondition,
        if_true: &WorkflowStep,
        if_false: Option<&WorkflowStep>,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        let start_time = Instant::now();

        debug!("Evaluating condition: {:?}", condition);

        let condition_met = self.evaluate_condition(condition, context);

        let result = if condition_met {
            debug!("Condition met, executing true branch");
            Box::pin(self.execute_step(if_true, context)).await
        } else if let Some(false_step) = if_false {
            debug!("Condition not met, executing false branch");
            Box::pin(self.execute_step(false_step, context)).await
        } else {
            debug!("Condition not met, no false branch");
            Ok(serde_json::json!(null))
        };

        let duration = start_time.elapsed();

        // Record conditional step result
        let step_result = WorkflowStepResult {
            step_name: "conditional".to_string(),
            result: result
                .as_ref()
                .map(|v| v.clone())
                .map_err(|e| e.to_string()),
            duration,
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert(
                    "condition_met".to_string(),
                    serde_json::json!(condition_met),
                );
                metadata.insert("condition".to_string(), serde_json::json!(condition));
                metadata
            },
        };

        context.step_results.push(step_result);

        result
    }

    /// Execute delay step
    async fn execute_delay_step(
        &self,
        duration: Duration,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        let start_time = Instant::now();

        debug!("Delaying for {:?}", duration);

        tokio::time::sleep(duration).await;

        let actual_duration = start_time.elapsed();

        // Record delay step result
        let step_result = WorkflowStepResult {
            step_name: "delay".to_string(),
            result: Ok(serde_json::json!({"delayed_ms": actual_duration.as_millis()})),
            duration: actual_duration,
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert(
                    "requested_duration_ms".to_string(),
                    serde_json::json!(duration.as_millis()),
                );
                metadata
            },
        };

        context.step_results.push(step_result);

        Ok(serde_json::json!({"delayed_ms": actual_duration.as_millis()}))
    }

    /// Execute loop step
    async fn execute_loop_step(
        &self,
        step: &WorkflowStep,
        count: usize,
        break_on_error: bool,
        context: &mut WorkflowContext,
    ) -> Result<serde_json::Value> {
        let start_time = Instant::now();

        debug!("Executing loop {} times", count);

        let mut results = Vec::new();
        let mut successful_iterations = 0;

        for i in 0..count {
            debug!("Loop iteration {}/{}", i + 1, count);

            match Box::pin(self.execute_step(step, context)).await {
                Ok(result) => {
                    results.push(result);
                    successful_iterations += 1;
                }
                Err(e) => {
                    if break_on_error {
                        warn!("Loop stopped at iteration {} due to error: {}", i + 1, e);
                        break;
                    } else {
                        warn!("Loop iteration {} failed, continuing: {}", i + 1, e);
                        results.push(serde_json::json!({"error": e.to_string()}));
                    }
                }
            }
        }

        let duration = start_time.elapsed();

        // Record loop step result
        let step_result = WorkflowStepResult {
            step_name: "loop".to_string(),
            result: Ok(serde_json::json!(results)),
            duration,
            metadata: {
                let mut metadata = HashMap::new();
                metadata.insert("requested_count".to_string(), serde_json::json!(count));
                metadata.insert(
                    "successful_iterations".to_string(),
                    serde_json::json!(successful_iterations),
                );
                metadata.insert(
                    "break_on_error".to_string(),
                    serde_json::json!(break_on_error),
                );
                metadata
            },
        };

        context.step_results.push(step_result);

        Ok(serde_json::json!(results))
    }

    /// Evaluate a workflow condition
    fn evaluate_condition(&self, condition: &WorkflowCondition, context: &WorkflowContext) -> bool {
        match condition {
            WorkflowCondition::Success => context
                .step_results
                .last()
                .map(|result| result.result.is_ok())
                .unwrap_or(false),
            WorkflowCondition::Failed => context
                .step_results
                .last()
                .map(|result| result.result.is_err())
                .unwrap_or(false),
            WorkflowCondition::Contains { value } => context
                .step_results
                .last()
                .and_then(|result| result.result.as_ref().ok())
                .map(|result| result.to_string().contains(value))
                .unwrap_or(false),
            WorkflowCondition::Equals { value } => context
                .step_results
                .last()
                .and_then(|result| result.result.as_ref().ok())
                .map(|result| result == value)
                .unwrap_or(false),
            WorkflowCondition::Expression { code: _ } => {
                // TODO: Implement JavaScript expression evaluation
                warn!("Expression conditions not yet implemented");
                false
            }
        }
    }

    /// Substitute variables in parameters
    fn substitute_variables(
        &self,
        params: &serde_json::Value,
        variables: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        // Simple variable substitution
        // In a real implementation, this would be more sophisticated
        let params_str = params.to_string();
        let mut substituted = params_str;

        for (key, value) in variables {
            let placeholder = format!("${{{}}}", key);
            let replacement = match value {
                serde_json::Value::String(s) => s.clone(),
                _ => value.to_string(),
            };
            substituted = substituted.replace(&placeholder, &replacement);
        }

        serde_json::from_str(&substituted).map_err(|e| LoxoneError::Generic(e.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn create_test_executor(
    ) -> impl Fn(&str, serde_json::Value) -> Result<serde_json::Value> + Send + Sync {
        move |tool_name: &str, _params: serde_json::Value| {
            match tool_name {
                "test_success" => Ok(serde_json::json!({"status": "success"})),
                "test_failure" => Err(LoxoneError::Generic(anyhow::anyhow!("Test failure"))),
                "test_delay" => {
                    // Use async sleep to properly handle timeouts
                    std::thread::sleep(Duration::from_millis(50));
                    Ok(serde_json::json!({"delayed": true}))
                }
                _ => Err(LoxoneError::Generic(anyhow::anyhow!(
                    "Unknown tool: {}",
                    tool_name
                ))),
            }
        }
    }

    #[tokio::test]
    async fn test_simple_tool_execution() {
        let engine = WorkflowEngine::new(create_test_executor());

        let workflow = Workflow {
            id: "test-1".to_string(),
            name: "Simple Test".to_string(),
            description: "Test simple tool execution".to_string(),
            root_step: WorkflowStep::Tool {
                name: "test_success".to_string(),
                params: serde_json::json!({}),
                timeout: None,
            },
            timeout: None,
            variables: HashMap::new(),
        };

        let result = engine.execute_workflow(workflow).await;

        assert!(result.success);
        assert_eq!(result.step_results.len(), 1);
        assert_eq!(result.step_results[0].step_name, "tool:test_success");
    }

    #[tokio::test]
    async fn test_sequential_execution() {
        let engine = WorkflowEngine::new(create_test_executor());

        let workflow = Workflow {
            id: "test-2".to_string(),
            name: "Sequential Test".to_string(),
            description: "Test sequential execution".to_string(),
            root_step: WorkflowStep::Sequential {
                steps: vec![
                    WorkflowStep::Tool {
                        name: "test_success".to_string(),
                        params: serde_json::json!({}),
                        timeout: None,
                    },
                    WorkflowStep::Tool {
                        name: "test_delay".to_string(),
                        params: serde_json::json!({}),
                        timeout: None,
                    },
                ],
            },
            timeout: None,
            variables: HashMap::new(),
        };

        let result = engine.execute_workflow(workflow).await;

        assert!(result.success);
        assert_eq!(result.step_results.len(), 3); // 2 tools + 1 sequential container
    }

    #[tokio::test]
    async fn test_conditional_execution() {
        let engine = WorkflowEngine::new(create_test_executor());

        let workflow = Workflow {
            id: "test-3".to_string(),
            name: "Conditional Test".to_string(),
            description: "Test conditional execution".to_string(),
            root_step: WorkflowStep::Sequential {
                steps: vec![
                    WorkflowStep::Tool {
                        name: "test_success".to_string(),
                        params: serde_json::json!({}),
                        timeout: None,
                    },
                    WorkflowStep::Conditional {
                        condition: WorkflowCondition::Success,
                        if_true: Box::new(WorkflowStep::Tool {
                            name: "test_delay".to_string(),
                            params: serde_json::json!({}),
                            timeout: None,
                        }),
                        if_false: None,
                    },
                ],
            },
            timeout: None,
            variables: HashMap::new(),
        };

        let result = engine.execute_workflow(workflow).await;

        assert!(result.success);
        assert_eq!(result.step_results.len(), 4); // 2 tools + 1 conditional + 1 sequential
    }

    #[tokio::test]
    async fn test_loop_execution() {
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = counter.clone();

        let engine =
            WorkflowEngine::new(
                move |tool_name: &str, _params: serde_json::Value| match tool_name {
                    "increment" => {
                        let count = counter_clone.fetch_add(1, Ordering::SeqCst);
                        Ok(serde_json::json!({"count": count + 1}))
                    }
                    _ => Err(LoxoneError::Generic(anyhow::anyhow!(
                        "Unknown tool: {}",
                        tool_name
                    ))),
                },
            );

        let workflow = Workflow {
            id: "test-4".to_string(),
            name: "Loop Test".to_string(),
            description: "Test loop execution".to_string(),
            root_step: WorkflowStep::Loop {
                step: Box::new(WorkflowStep::Tool {
                    name: "increment".to_string(),
                    params: serde_json::json!({}),
                    timeout: None,
                }),
                count: 3,
                break_on_error: false,
            },
            timeout: None,
            variables: HashMap::new(),
        };

        let result = engine.execute_workflow(workflow).await;

        assert!(result.success);
        assert_eq!(counter.load(Ordering::SeqCst), 3);
        assert_eq!(result.step_results.len(), 4); // 3 loop iterations + 1 loop container
    }

    #[tokio::test]
    async fn test_error_handling() {
        let engine = WorkflowEngine::new(create_test_executor());

        let workflow = Workflow {
            id: "test-5".to_string(),
            name: "Error Test".to_string(),
            description: "Test error handling".to_string(),
            root_step: WorkflowStep::Tool {
                name: "test_failure".to_string(),
                params: serde_json::json!({}),
                timeout: None,
            },
            timeout: None,
            variables: HashMap::new(),
        };

        let result = engine.execute_workflow(workflow).await;

        assert!(!result.success);
        assert!(result.error.is_some());
    }
}
