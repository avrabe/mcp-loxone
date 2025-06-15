//! MCP service infrastructure for transport and lifecycle management

use crate::{
    error::{Error, Result},
    model::*,
    server::{RequestContext, ServerHandler},
};
use async_trait::async_trait;
use futures_util::{sink::SinkExt, stream::StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader},
    sync::mpsc,
};
use tokio_util::codec::{FramedRead, FramedWrite, LinesCodec};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// JSON-RPC request structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// JSON-RPC response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC error structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

/// Service quit reason
#[derive(Debug)]
pub enum QuitReason {
    ClientDisconnected,
    ServerError(Error),
    Shutdown,
}

/// MCP service for handling transport and protocol
pub struct McpService<H: ServerHandler> {
    handler: H,
    receiver: mpsc::UnboundedReceiver<QuitReason>,
}

impl<H: ServerHandler> McpService<H> {
    /// Create a new service with the given handler
    pub fn new(handler: H) -> (Self, mpsc::UnboundedSender<QuitReason>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                handler,
                receiver: rx,
            },
            tx,
        )
    }

    /// Wait for the service to quit
    pub async fn waiting(mut self) -> Result<QuitReason> {
        match self.receiver.recv().await {
            Some(reason) => Ok(reason),
            None => Ok(QuitReason::Shutdown),
        }
    }

    /// Handle a JSON-RPC request
    async fn handle_request(&self, request: JsonRpcRequest) -> JsonRpcResponse {
        let context = RequestContext::with_id(Uuid::new_v4());

        debug!("Handling MCP request: {}", request.method);

        let result = match request.method.as_str() {
            "ping" => match self.handler.ping(context).await {
                Ok(()) => Ok(Value::Object(serde_json::Map::new())),
                Err(e) => Err(e),
            },
            "initialize" => match request.params {
                Some(params) => match serde_json::from_value::<InitializeRequestParam>(params) {
                    Ok(params) => match self.handler.initialize(params, context).await {
                        Ok(result) => serde_json::to_value(result).map_err(Error::from),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                },
                None => Err(Error::invalid_params("Missing parameters for initialize")),
            },
            "tools/list" => {
                let params = request.params.unwrap_or(Value::Null);
                match serde_json::from_value::<PaginatedRequestParam>(params) {
                    Ok(params) => match self.handler.list_tools(params, context).await {
                        Ok(result) => serde_json::to_value(result).map_err(Error::from),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                }
            }
            "tools/call" => match request.params {
                Some(params) => match serde_json::from_value::<CallToolRequestParam>(params) {
                    Ok(params) => match self.handler.call_tool(params, context).await {
                        Ok(result) => serde_json::to_value(result).map_err(Error::from),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                },
                None => Err(Error::invalid_params("Missing parameters for tools/call")),
            },
            "resources/list" => {
                let params = request.params.unwrap_or(Value::Null);
                match serde_json::from_value::<PaginatedRequestParam>(params) {
                    Ok(params) => match self.handler.list_resources(params, context).await {
                        Ok(result) => serde_json::to_value(result).map_err(Error::from),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                }
            }
            "resources/read" => match request.params {
                Some(params) => match serde_json::from_value::<ReadResourceRequestParam>(params) {
                    Ok(params) => match self.handler.read_resource(params, context).await {
                        Ok(result) => serde_json::to_value(result).map_err(Error::from),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                },
                None => Err(Error::invalid_params(
                    "Missing parameters for resources/read",
                )),
            },
            "prompts/list" => {
                let params = request.params.unwrap_or(Value::Null);
                match serde_json::from_value::<PaginatedRequestParam>(params) {
                    Ok(params) => match self.handler.list_prompts(params, context).await {
                        Ok(result) => serde_json::to_value(result).map_err(Error::from),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                }
            }
            "prompts/get" => match request.params {
                Some(params) => match serde_json::from_value::<GetPromptRequestParam>(params) {
                    Ok(params) => match self.handler.get_prompt(params, context).await {
                        Ok(result) => serde_json::to_value(result).map_err(Error::from),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                },
                None => Err(Error::invalid_params("Missing parameters for prompts/get")),
            },
            "completion/complete" => match request.params {
                Some(params) => match serde_json::from_value::<CompleteRequestParam>(params) {
                    Ok(params) => match self.handler.complete(params, context).await {
                        Ok(result) => serde_json::to_value(result).map_err(Error::from),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                },
                None => Err(Error::invalid_params(
                    "Missing parameters for completion/complete",
                )),
            },
            "logging/setLevel" => match request.params {
                Some(params) => match serde_json::from_value::<SetLevelRequestParam>(params) {
                    Ok(params) => match self.handler.set_level(params, context).await {
                        Ok(()) => Ok(Value::Object(serde_json::Map::new())),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                },
                None => Err(Error::invalid_params(
                    "Missing parameters for logging/setLevel",
                )),
            },
            "resources/templates/list" => {
                let params = request.params.unwrap_or(Value::Null);
                match serde_json::from_value::<PaginatedRequestParam>(params) {
                    Ok(params) => {
                        match self.handler.list_resource_templates(params, context).await {
                            Ok(result) => serde_json::to_value(result).map_err(Error::from),
                            Err(e) => Err(e),
                        }
                    }
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                }
            }
            "resources/subscribe" => match request.params {
                Some(params) => match serde_json::from_value::<SubscribeRequestParam>(params) {
                    Ok(params) => match self.handler.subscribe(params, context).await {
                        Ok(()) => Ok(Value::Object(serde_json::Map::new())),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                },
                None => Err(Error::invalid_params(
                    "Missing parameters for resources/subscribe",
                )),
            },
            "resources/unsubscribe" => match request.params {
                Some(params) => match serde_json::from_value::<UnsubscribeRequestParam>(params) {
                    Ok(params) => match self.handler.unsubscribe(params, context).await {
                        Ok(()) => Ok(Value::Object(serde_json::Map::new())),
                        Err(e) => Err(e),
                    },
                    Err(e) => Err(Error::invalid_params(e.to_string())),
                },
                None => Err(Error::invalid_params(
                    "Missing parameters for resources/unsubscribe",
                )),
            },
            _ => Err(Error::method_not_found(request.method)),
        };

        match result {
            Ok(result) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: Some(result),
                error: None,
            },
            Err(error) => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id: request.id,
                result: None,
                error: Some(JsonRpcError {
                    code: error.json_rpc_code(),
                    message: error.to_string(),
                    data: None,
                }),
            },
        }
    }
}

/// Service extension trait for starting MCP services
#[async_trait]
pub trait ServiceExt: ServerHandler + Sized + 'static {
    /// Serve MCP protocol over the given I/O stream
    async fn serve<IO>(self, io: IO) -> Result<McpService<Self>>
    where
        IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (service, quit_tx) = McpService::new(self.clone());

        // Split the I/O stream
        let (reader, writer) = tokio::io::split(io);
        let mut lines_read = FramedRead::new(reader, LinesCodec::new());
        let mut lines_write = FramedWrite::new(writer, LinesCodec::new());

        // Handle the protocol in a background task
        let handler = self.clone();
        let quit_tx_clone = quit_tx.clone();

        tokio::spawn(async move {
            info!("MCP service started on stdio transport");

            while let Some(line_result) = lines_read.next().await {
                match line_result {
                    Ok(line) => {
                        if line.trim().is_empty() {
                            continue;
                        }

                        debug!("Received request: {}", line);

                        // Parse JSON-RPC request
                        match serde_json::from_str::<JsonRpcRequest>(&line) {
                            Ok(request) => {
                                // Handle the request
                                let temp_service = McpService::new(handler.clone()).0;
                                let response = temp_service.handle_request(request).await;

                                // Send response
                                match serde_json::to_string(&response) {
                                    Ok(response_line) => {
                                        debug!("Sending response: {}", response_line);
                                        if let Err(e) = lines_write.send(response_line).await {
                                            error!("Failed to send response: {}", e);
                                            let _ = quit_tx_clone.send(QuitReason::ServerError(
                                                Error::connection_error(e.to_string()),
                                            ));
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to serialize response: {}", e);
                                        let _ = quit_tx_clone.send(QuitReason::ServerError(
                                            Error::parse_error(e.to_string()),
                                        ));
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse JSON-RPC request: {}", e);
                                // Send parse error response
                                let error_response = JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: None,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: -32700,
                                        message: format!("Parse error: {}", e),
                                        data: None,
                                    }),
                                };

                                if let Ok(response_line) = serde_json::to_string(&error_response) {
                                    if let Err(e) = lines_write.send(response_line).await {
                                        error!("Failed to send error response: {}", e);
                                        let _ = quit_tx_clone.send(QuitReason::ServerError(
                                            Error::connection_error(e.to_string()),
                                        ));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("I/O error reading request: {}", e);
                        let _ = quit_tx_clone.send(QuitReason::ClientDisconnected);
                        break;
                    }
                }
            }

            info!("MCP service protocol handler finished");
            let _ = quit_tx_clone.send(QuitReason::ClientDisconnected);
        });

        Ok(service)
    }

    /// Serve MCP protocol over stdio (convenience method)
    async fn serve_stdio(self) -> Result<McpService<Self>> {
        let (service, quit_tx) = McpService::new(self.clone());

        // Handle the protocol in a background task using stdio
        let handler = self.clone();
        let quit_tx_clone = quit_tx.clone();

        tokio::spawn(async move {
            info!("MCP service started on stdio transport");

            let stdin = tokio::io::stdin();
            let stdout = tokio::io::stdout();
            let mut stdin = BufReader::new(stdin);
            let mut stdout = stdout;

            let mut line = String::new();

            loop {
                line.clear();
                match stdin.read_line(&mut line).await {
                    Ok(0) => {
                        // EOF
                        debug!("Stdin EOF reached");
                        break;
                    }
                    Ok(_) => {
                        let line = line.trim();
                        if line.is_empty() {
                            continue;
                        }

                        debug!("Received request: {}", line);

                        // Parse JSON-RPC request
                        match serde_json::from_str::<JsonRpcRequest>(line) {
                            Ok(request) => {
                                // Handle the request
                                let temp_service = McpService::new(handler.clone()).0;
                                let response = temp_service.handle_request(request).await;

                                // Send response
                                match serde_json::to_string(&response) {
                                    Ok(response_line) => {
                                        debug!("Sending response: {}", response_line);
                                        if let Err(e) = stdout
                                            .write_all((response_line + "\n").as_bytes())
                                            .await
                                        {
                                            error!("Failed to send response: {}", e);
                                            let _ = quit_tx_clone.send(QuitReason::ServerError(
                                                Error::connection_error(e.to_string()),
                                            ));
                                            break;
                                        }
                                        if let Err(e) = stdout.flush().await {
                                            error!("Failed to flush stdout: {}", e);
                                            let _ = quit_tx_clone.send(QuitReason::ServerError(
                                                Error::connection_error(e.to_string()),
                                            ));
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        error!("Failed to serialize response: {}", e);
                                        let _ = quit_tx_clone.send(QuitReason::ServerError(
                                            Error::parse_error(e.to_string()),
                                        ));
                                        break;
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("Failed to parse JSON-RPC request: {}", e);
                                // Send parse error response
                                let error_response = JsonRpcResponse {
                                    jsonrpc: "2.0".to_string(),
                                    id: None,
                                    result: None,
                                    error: Some(JsonRpcError {
                                        code: -32700,
                                        message: format!("Parse error: {}", e),
                                        data: None,
                                    }),
                                };

                                if let Ok(response_line) = serde_json::to_string(&error_response) {
                                    if let Err(e) =
                                        stdout.write_all((response_line + "\n").as_bytes()).await
                                    {
                                        error!("Failed to send error response: {}", e);
                                        let _ = quit_tx_clone.send(QuitReason::ServerError(
                                            Error::connection_error(e.to_string()),
                                        ));
                                        break;
                                    }
                                    if let Err(e) = stdout.flush().await {
                                        error!("Failed to flush stdout: {}", e);
                                        let _ = quit_tx_clone.send(QuitReason::ServerError(
                                            Error::connection_error(e.to_string()),
                                        ));
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!("I/O error reading request: {}", e);
                        let _ = quit_tx_clone.send(QuitReason::ClientDisconnected);
                        break;
                    }
                }
            }

            info!("MCP service stdio handler finished");
            let _ = quit_tx_clone.send(QuitReason::ClientDisconnected);
        });

        Ok(service)
    }
}
