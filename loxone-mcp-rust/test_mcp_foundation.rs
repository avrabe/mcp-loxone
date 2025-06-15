#!/usr/bin/env rust-script
//! This is a test script to verify mcp-foundation is working
//! 
//! Run with: cargo +nightly -Zscript test_mcp_foundation.rs

use mcp_foundation::{ServerHandler, ServiceExt, ServerInfo, ServerCapabilities, Implementation, ProtocolVersion};
use async_trait::async_trait;

#[derive(Clone)]
struct TestServer;

#[async_trait]
impl ServerHandler for TestServer {
    async fn ping(&self, _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>) -> mcp_foundation::Result<()> {
        println!("Ping received!");
        Ok(())
    }

    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation {
                name: "Test Server".into(),
                version: "1.0.0".into(),
            },
            instructions: Some("Test MCP server".into()),
        }
    }

    async fn list_tools(
        &self,
        _request: mcp_foundation::PaginatedRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<mcp_foundation::ListToolsResult> {
        Ok(mcp_foundation::ListToolsResult {
            tools: vec![],
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        _request: mcp_foundation::CallToolRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<mcp_foundation::CallToolResult> {
        Ok(mcp_foundation::CallToolResult::text("Not implemented"))
    }

    async fn list_resources(
        &self,
        _request: mcp_foundation::PaginatedRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<mcp_foundation::ListResourcesResult> {
        Ok(mcp_foundation::ListResourcesResult {
            resources: vec![],
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        _request: mcp_foundation::ReadResourceRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<mcp_foundation::ReadResourceResult> {
        Err(mcp_foundation::Error::resource_not_found("test"))
    }

    async fn list_prompts(
        &self,
        _request: mcp_foundation::PaginatedRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<mcp_foundation::ListPromptsResult> {
        Ok(mcp_foundation::ListPromptsResult {
            prompts: vec![],
            next_cursor: None,
        })
    }

    async fn get_prompt(
        &self,
        _request: mcp_foundation::GetPromptRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<mcp_foundation::GetPromptResult> {
        Err(mcp_foundation::Error::method_not_found("get_prompt"))
    }

    async fn initialize(
        &self,
        _request: mcp_foundation::InitializeRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<mcp_foundation::InitializeResult> {
        Ok(mcp_foundation::InitializeResult {
            protocol_version: "2024-11-05".into(),
            capabilities: ServerCapabilities::default(),
            server_info: Implementation {
                name: "Test".into(),
                version: "1.0.0".into(),
            },
            instructions: None,
        })
    }

    async fn complete(
        &self,
        _request: mcp_foundation::CompleteRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<mcp_foundation::CompleteResult> {
        Ok(mcp_foundation::CompleteResult::simple(""))
    }

    async fn set_level(
        &self,
        _request: mcp_foundation::SetLevelRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<()> {
        Ok(())
    }

    async fn list_resource_templates(
        &self,
        _request: mcp_foundation::PaginatedRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<mcp_foundation::ListResourceTemplatesResult> {
        Ok(mcp_foundation::ListResourceTemplatesResult {
            resource_templates: vec![],
            next_cursor: None,
        })
    }

    async fn subscribe(
        &self,
        _request: mcp_foundation::SubscribeRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<()> {
        Ok(())
    }

    async fn unsubscribe(
        &self,
        _request: mcp_foundation::UnsubscribeRequestParam,
        _context: mcp_foundation::RequestContext<mcp_foundation::RoleServer>,
    ) -> mcp_foundation::Result<()> {
        Ok(())
    }
}

impl ServiceExt for TestServer {}

#[tokio::main]
async fn main() {
    println!("Testing mcp-foundation integration...");
    
    let server = TestServer;
    let info = server.get_info();
    println!("Server info: {:?}", info);
    
    println!("✅ mcp-foundation is working!");
}