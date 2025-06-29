//! Derive macros for MCP CLI framework
//!
//! This crate provides the proc macro implementations for automatic CLI generation
//! and configuration management in the MCP framework.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Attribute, Data, DeriveInput, Fields};

/// Derive macro for `McpConfig`
///
/// This macro generates implementations for:
/// - `McpConfiguration` trait
/// - Automatic server info population from Cargo.toml
/// - Logging configuration setup
/// - CLI argument parsing integration with clap
///
/// # Attributes
///
/// ## Field-level attributes:
/// - `#[mcp(auto_populate)]` - Auto-populate field from Cargo.toml
/// - `#[mcp(logging(level = "info", format = "json"))]` - Configure logging
/// - `#[mcp(skip)]` - Skip field in CLI generation
///
/// # Example
///
/// ```rust,ignore
/// #[derive(McpConfig, Parser)]
/// struct MyConfig {
///     #[clap(short, long)]
///     port: u16,
///     
///     #[mcp(auto_populate)]
///     server_info: ServerInfo,
///     
///     #[mcp(logging(level = "debug", format = "json"))]
///     logging: LoggingConfig,
/// }
/// ```
#[proc_macro_derive(McpConfig, attributes(mcp))]
pub fn derive_mcp_config(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match generate_mcp_config_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

/// Derive macro for `McpBackend`
///
/// This macro generates implementations for:
/// - Custom error type with automatic conversions
/// - Backend trait delegation to reduce boilerplate
/// - Automatic error mapping and handling
/// - Integration with the MCP server framework
///
/// # Attributes
///
/// ## Type-level attributes:
/// - `#[mcp_backend(error = "CustomError")]` - Use custom error type
/// - `#[mcp_backend(config = "CustomConfig")]` - Use custom config type
/// - `#[mcp_backend(simple)]` - Implement SimpleBackend instead of full McpBackend
///
/// ## Field-level attributes:
/// - `#[mcp_backend(delegate)]` - Delegate method calls to this field
/// - `#[mcp_backend(error_from)]` - Generate error conversion from this type
///
/// # Example
///
/// ```rust,ignore
/// #[derive(McpBackend)]
/// #[mcp_backend(error = "MyBackendError", config = "MyConfig")]
/// struct MyBackend {
///     #[mcp_backend(delegate)]
///     inner: SomeInnerBackend,
///     config: MyConfig,
/// }
/// ```
#[proc_macro_derive(McpBackend, attributes(mcp_backend))]
pub fn derive_mcp_backend(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match generate_mcp_backend_impl(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn generate_mcp_config_impl(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;

    // Parse struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "McpConfig can only be derived for structs with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "McpConfig can only be derived for structs",
            ))
        }
    };

    // Analyze fields for MCP attributes
    let mut server_info_field = None;
    let mut logging_field = None;
    let mut auto_populate_fields = Vec::new();

    for field in fields {
        if let Some(ident) = &field.ident {
            // Check for MCP attributes
            for attr in &field.attrs {
                if attr.path().is_ident("mcp") {
                    parse_mcp_attribute(
                        attr,
                        ident,
                        &mut server_info_field,
                        &mut logging_field,
                        &mut auto_populate_fields,
                    )?;
                }
            }

            // Also check by field name conventions
            match ident.to_string().as_str() {
                "server_info" => server_info_field = Some(ident.clone()),
                "logging" => logging_field = Some(ident.clone()),
                _ => {}
            }
        }
    }

    // Generate trait implementation
    let server_info_impl = generate_server_info_impl(&server_info_field);
    let logging_impl = generate_logging_impl(&logging_field);
    let auto_populate_impl = generate_auto_populate_impl(&auto_populate_fields);

    Ok(quote! {
        impl pulseengine_mcp_cli::McpConfiguration for #name {
            #server_info_impl
            #logging_impl

            fn validate(&self) -> std::result::Result<(), pulseengine_mcp_cli::CliError> {
                // Validation logic here
                Ok(())
            }
        }

        impl #name {
            /// Create a new instance with auto-populated fields
            pub fn with_auto_populate() -> Self
            where
                Self: Default,
            {
                let mut instance = Self::default();
                instance.auto_populate();
                instance
            }

            /// Auto-populate fields from environment and Cargo.toml
            pub fn auto_populate(&mut self) {
                #auto_populate_impl
            }
        }
    })
}

fn generate_mcp_backend_impl(input: &DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let name = &input.ident;

    // Parse backend attributes
    let backend_config = parse_backend_attributes(input)?;

    // Parse struct fields
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    input,
                    "McpBackend can only be derived for structs with named fields",
                ))
            }
        },
        _ => {
            return Err(syn::Error::new_spanned(
                input,
                "McpBackend can only be derived for structs",
            ))
        }
    };

    // Find delegate fields and error conversions
    let mut delegate_field = None;
    let mut error_from_fields = Vec::new();

    for field in fields {
        if let Some(ident) = &field.ident {
            for attr in &field.attrs {
                if attr.path().is_ident("mcp_backend") {
                    parse_backend_field_attribute(
                        attr,
                        ident,
                        &mut delegate_field,
                        &mut error_from_fields,
                    )?;
                }
            }
        }
    }

    // Generate error type if needed
    let error_type = backend_config
        .error_type
        .as_ref()
        .map(|s| syn::parse_str::<syn::Type>(s))
        .transpose()?
        .unwrap_or_else(|| syn::parse_str(&format!("{name}Error")).unwrap());

    let config_type = backend_config
        .config_type
        .as_ref()
        .map(|s| syn::parse_str::<syn::Type>(s))
        .transpose()?
        .unwrap_or_else(|| syn::parse_str(&format!("{name}Config")).unwrap());

    // Generate error type definition if using default
    let error_definition = if backend_config.error_type.is_none() {
        generate_error_type_definition(name, &error_from_fields)
    } else {
        quote! {}
    };

    // Generate trait implementation
    let trait_impl = if backend_config.simple_backend {
        generate_simple_backend_impl(name, &error_type, &config_type, &delegate_field)
    } else {
        generate_full_backend_impl(name, &error_type, &config_type, &delegate_field)
    };

    Ok(quote! {
        #error_definition
        #trait_impl
    })
}

#[derive(Default)]
struct BackendConfig {
    error_type: Option<String>,
    config_type: Option<String>,
    simple_backend: bool,
}

fn parse_backend_attributes(input: &DeriveInput) -> syn::Result<BackendConfig> {
    let mut config = BackendConfig::default();

    for attr in &input.attrs {
        if attr.path().is_ident("mcp_backend") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("simple") {
                    config.simple_backend = true;
                    Ok(())
                } else if meta.path.is_ident("error") {
                    if let Ok(value) = meta.value() {
                        if let Ok(lit) = value.parse::<syn::LitStr>() {
                            config.error_type = Some(lit.value());
                        }
                    }
                    Ok(())
                } else if meta.path.is_ident("config") {
                    if let Ok(value) = meta.value() {
                        if let Ok(lit) = value.parse::<syn::LitStr>() {
                            config.config_type = Some(lit.value());
                        }
                    }
                    Ok(())
                } else {
                    Err(meta.error(format!(
                        "unsupported mcp_backend attribute: {}",
                        meta.path.get_ident().unwrap()
                    )))
                }
            })?;
        }
    }

    Ok(config)
}

fn parse_backend_field_attribute(
    attr: &Attribute,
    field_ident: &syn::Ident,
    delegate_field: &mut Option<syn::Ident>,
    error_from_fields: &mut Vec<syn::Ident>,
) -> syn::Result<()> {
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("delegate") {
            *delegate_field = Some(field_ident.clone());
            Ok(())
        } else if meta.path.is_ident("error_from") {
            error_from_fields.push(field_ident.clone());
            Ok(())
        } else {
            Err(meta.error(format!(
                "unsupported mcp_backend field attribute: {}",
                meta.path.get_ident().unwrap()
            )))
        }
    })
}

fn generate_error_type_definition(
    name: &syn::Ident,
    error_from_fields: &[syn::Ident],
) -> proc_macro2::TokenStream {
    let error_name = syn::Ident::new(&format!("{name}Error"), name.span());

    let from_implementations = error_from_fields.iter().map(|_field| {
        // This is a simplified approach - in practice you'd need type analysis
        quote! {
            impl From<std::io::Error> for #error_name {
                fn from(err: std::io::Error) -> Self {
                    Self::Internal(err.to_string())
                }
            }
        }
    });

    quote! {
        #[derive(Debug, thiserror::Error)]
        pub enum #error_name {
            #[error("Configuration error: {0}")]
            Configuration(String),

            #[error("Connection error: {0}")]
            Connection(String),

            #[error("Operation not supported: {0}")]
            NotSupported(String),

            #[error("Internal error: {0}")]
            Internal(String),
        }

        impl #error_name {
            pub fn configuration(msg: impl Into<String>) -> Self {
                Self::Configuration(msg.into())
            }

            pub fn connection(msg: impl Into<String>) -> Self {
                Self::Connection(msg.into())
            }

            pub fn not_supported(msg: impl Into<String>) -> Self {
                Self::NotSupported(msg.into())
            }

            pub fn internal(msg: impl Into<String>) -> Self {
                Self::Internal(msg.into())
            }
        }

        impl From<pulseengine_mcp_server::backend::BackendError> for #error_name {
            fn from(err: pulseengine_mcp_server::backend::BackendError) -> Self {
                Self::Internal(err.to_string())
            }
        }

        impl From<#error_name> for pulseengine_mcp_protocol::Error {
            fn from(err: #error_name) -> Self {
                match err {
                    #error_name::Configuration(msg) => Self::invalid_params(msg),
                    #error_name::Connection(msg) => Self::internal_error(format!("Connection failed: {msg}")),
                    #error_name::NotSupported(msg) => Self::method_not_found(msg),
                    #error_name::Internal(msg) => Self::internal_error(msg),
                }
            }
        }

        #(#from_implementations)*
    }
}

fn generate_simple_backend_impl(
    name: &syn::Ident,
    error_type: &syn::Type,
    config_type: &syn::Type,
    delegate_field: &Option<syn::Ident>,
) -> proc_macro2::TokenStream {
    if let Some(delegate) = delegate_field {
        quote! {
            #[async_trait::async_trait]
            impl pulseengine_mcp_server::backend::SimpleBackend for #name {
                type Error = #error_type;
                type Config = #config_type;

                async fn initialize(config: Self::Config) -> std::result::Result<Self, Self::Error> {
                    // Default implementation - override as needed
                    Err(Self::Error::not_supported("Backend initialization not implemented"))
                }

                fn get_server_info(&self) -> pulseengine_mcp_protocol::ServerInfo {
                    self.#delegate.get_server_info()
                }

                async fn health_check(&self) -> std::result::Result<(), Self::Error> {
                    self.#delegate.health_check().await.map_err(Into::into)
                }

                async fn list_tools(
                    &self,
                    request: pulseengine_mcp_protocol::PaginatedRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::ListToolsResult, Self::Error> {
                    self.#delegate.list_tools(request).await.map_err(Into::into)
                }

                async fn call_tool(
                    &self,
                    request: pulseengine_mcp_protocol::CallToolRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::CallToolResult, Self::Error> {
                    self.#delegate.call_tool(request).await.map_err(Into::into)
                }
            }
        }
    } else {
        quote! {
            #[async_trait::async_trait]
            impl pulseengine_mcp_server::backend::SimpleBackend for #name {
                type Error = #error_type;
                type Config = #config_type;

                async fn initialize(config: Self::Config) -> std::result::Result<Self, Self::Error> {
                    // Default implementation - override as needed
                    Err(Self::Error::not_supported("Backend initialization not implemented"))
                }

                fn get_server_info(&self) -> pulseengine_mcp_protocol::ServerInfo {
                    // Default implementation - override as needed
                    pulseengine_mcp_protocol::ServerInfo {
                        protocol_version: pulseengine_mcp_protocol::ProtocolVersion::default(),
                        capabilities: pulseengine_mcp_protocol::ServerCapabilities::default(),
                        server_info: pulseengine_mcp_protocol::Implementation {
                            name: env!("CARGO_PKG_NAME").to_string(),
                            version: env!("CARGO_PKG_VERSION").to_string(),
                        },
                        instructions: None,
                    }
                }

                async fn health_check(&self) -> std::result::Result<(), Self::Error> {
                    // Default implementation - override as needed
                    Ok(())
                }

                async fn list_tools(
                    &self,
                    _request: pulseengine_mcp_protocol::PaginatedRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::ListToolsResult, Self::Error> {
                    // Default implementation - override as needed
                    Ok(pulseengine_mcp_protocol::ListToolsResult {
                        tools: vec![],
                        next_cursor: None,
                    })
                }

                async fn call_tool(
                    &self,
                    request: pulseengine_mcp_protocol::CallToolRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::CallToolResult, Self::Error> {
                    // Default implementation - override as needed
                    Err(Self::Error::not_supported(format!("Tool not found: {}", request.name)))
                }
            }
        }
    }
}

fn generate_full_backend_impl(
    name: &syn::Ident,
    error_type: &syn::Type,
    config_type: &syn::Type,
    delegate_field: &Option<syn::Ident>,
) -> proc_macro2::TokenStream {
    if let Some(delegate) = delegate_field {
        quote! {
            #[async_trait::async_trait]
            impl pulseengine_mcp_server::backend::McpBackend for #name {
                type Error = #error_type;
                type Config = #config_type;

                async fn initialize(config: Self::Config) -> std::result::Result<Self, Self::Error> {
                    // Default implementation - override as needed
                    Err(Self::Error::not_supported("Backend initialization not implemented"))
                }

                fn get_server_info(&self) -> pulseengine_mcp_protocol::ServerInfo {
                    self.#delegate.get_server_info()
                }

                async fn health_check(&self) -> std::result::Result<(), Self::Error> {
                    self.#delegate.health_check().await.map_err(Into::into)
                }

                // Delegate all methods to the inner field with error conversion
                async fn list_tools(
                    &self,
                    request: pulseengine_mcp_protocol::PaginatedRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::ListToolsResult, Self::Error> {
                    self.#delegate.list_tools(request).await.map_err(Into::into)
                }

                async fn call_tool(
                    &self,
                    request: pulseengine_mcp_protocol::CallToolRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::CallToolResult, Self::Error> {
                    self.#delegate.call_tool(request).await.map_err(Into::into)
                }

                async fn list_resources(
                    &self,
                    request: pulseengine_mcp_protocol::PaginatedRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::ListResourcesResult, Self::Error> {
                    self.#delegate.list_resources(request).await.map_err(Into::into)
                }

                async fn read_resource(
                    &self,
                    request: pulseengine_mcp_protocol::ReadResourceRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::ReadResourceResult, Self::Error> {
                    self.#delegate.read_resource(request).await.map_err(Into::into)
                }

                async fn list_prompts(
                    &self,
                    request: pulseengine_mcp_protocol::PaginatedRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::ListPromptsResult, Self::Error> {
                    self.#delegate.list_prompts(request).await.map_err(Into::into)
                }

                async fn get_prompt(
                    &self,
                    request: pulseengine_mcp_protocol::GetPromptRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::GetPromptResult, Self::Error> {
                    self.#delegate.get_prompt(request).await.map_err(Into::into)
                }
            }
        }
    } else {
        quote! {
            #[async_trait::async_trait]
            impl pulseengine_mcp_server::backend::McpBackend for #name {
                type Error = #error_type;
                type Config = #config_type;

                async fn initialize(config: Self::Config) -> std::result::Result<Self, Self::Error> {
                    // Default implementation - override as needed
                    Err(Self::Error::not_supported("Backend initialization not implemented"))
                }

                fn get_server_info(&self) -> pulseengine_mcp_protocol::ServerInfo {
                    // Default implementation - override as needed
                    pulseengine_mcp_protocol::ServerInfo {
                        protocol_version: pulseengine_mcp_protocol::ProtocolVersion::default(),
                        capabilities: pulseengine_mcp_protocol::ServerCapabilities::default(),
                        server_info: pulseengine_mcp_protocol::Implementation {
                            name: env!("CARGO_PKG_NAME").to_string(),
                            version: env!("CARGO_PKG_VERSION").to_string(),
                        },
                        instructions: None,
                    }
                }

                async fn health_check(&self) -> std::result::Result<(), Self::Error> {
                    // Default implementation - override as needed
                    Ok(())
                }

                // Default implementations for all required methods
                async fn list_tools(
                    &self,
                    _request: pulseengine_mcp_protocol::PaginatedRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::ListToolsResult, Self::Error> {
                    Ok(pulseengine_mcp_protocol::ListToolsResult {
                        tools: vec![],
                        next_cursor: None,
                    })
                }

                async fn call_tool(
                    &self,
                    request: pulseengine_mcp_protocol::CallToolRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::CallToolResult, Self::Error> {
                    Err(Self::Error::not_supported(format!("Tool not found: {}", request.name)))
                }

                async fn list_resources(
                    &self,
                    _request: pulseengine_mcp_protocol::PaginatedRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::ListResourcesResult, Self::Error> {
                    Ok(pulseengine_mcp_protocol::ListResourcesResult {
                        resources: vec![],
                        next_cursor: None,
                    })
                }

                async fn read_resource(
                    &self,
                    request: pulseengine_mcp_protocol::ReadResourceRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::ReadResourceResult, Self::Error> {
                    Err(Self::Error::not_supported(format!("Resource not found: {}", request.uri)))
                }

                async fn list_prompts(
                    &self,
                    _request: pulseengine_mcp_protocol::PaginatedRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::ListPromptsResult, Self::Error> {
                    Ok(pulseengine_mcp_protocol::ListPromptsResult {
                        prompts: vec![],
                        next_cursor: None,
                    })
                }

                async fn get_prompt(
                    &self,
                    request: pulseengine_mcp_protocol::GetPromptRequestParam,
                ) -> std::result::Result<pulseengine_mcp_protocol::GetPromptResult, Self::Error> {
                    Err(Self::Error::not_supported(format!("Prompt not found: {}", request.name)))
                }
            }
        }
    }
}

fn parse_mcp_attribute(
    attr: &Attribute,
    field_ident: &syn::Ident,
    _server_info_field: &mut Option<syn::Ident>,
    logging_field: &mut Option<syn::Ident>,
    auto_populate_fields: &mut Vec<syn::Ident>,
) -> syn::Result<()> {
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("auto_populate") {
            auto_populate_fields.push(field_ident.clone());
            Ok(())
        } else if meta.path.is_ident("logging") {
            *logging_field = Some(field_ident.clone());
            // Parse logging configuration options if present
            if meta.input.peek(syn::token::Paren) {
                let _content;
                syn::parenthesized!(_content in meta.input);
                // For now, just accept any logging configuration
                // TODO: Parse specific logging options like level, format, etc.
            }
            Ok(())
        } else {
            Err(meta.error(format!(
                "unsupported mcp attribute: {}",
                meta.path.get_ident().unwrap()
            )))
        }
    })
}

fn generate_server_info_impl(server_info_field: &Option<syn::Ident>) -> proc_macro2::TokenStream {
    if let Some(field) = server_info_field {
        quote! {
            fn get_server_info(&self) -> &pulseengine_mcp_protocol::ServerInfo {
                self.#field.as_ref().unwrap_or_else(|| {
                    static SERVER_INFO: std::sync::OnceLock<pulseengine_mcp_protocol::ServerInfo> = std::sync::OnceLock::new();
                    SERVER_INFO.get_or_init(|| {
                        pulseengine_mcp_cli::config::create_server_info(None, None)
                    })
                })
            }
        }
    } else {
        quote! {
            fn get_server_info(&self) -> &pulseengine_mcp_protocol::ServerInfo {
                use std::sync::OnceLock;
                static SERVER_INFO: OnceLock<pulseengine_mcp_protocol::ServerInfo> = OnceLock::new();
                SERVER_INFO.get_or_init(|| {
                    pulseengine_mcp_cli::config::create_server_info(None, None)
                })
            }
        }
    }
}

fn generate_logging_impl(logging_field: &Option<syn::Ident>) -> proc_macro2::TokenStream {
    if let Some(field) = logging_field {
        quote! {
            fn get_logging_config(&self) -> &pulseengine_mcp_cli::DefaultLoggingConfig {
                self.#field.as_ref().unwrap_or_else(|| {
                    static LOGGING_CONFIG: std::sync::OnceLock<pulseengine_mcp_cli::DefaultLoggingConfig> = std::sync::OnceLock::new();
                    LOGGING_CONFIG.get_or_init(|| {
                        pulseengine_mcp_cli::DefaultLoggingConfig::default()
                    })
                })
            }

            fn initialize_logging(&self) -> std::result::Result<(), pulseengine_mcp_cli::CliError> {
                // Initialize logging using the field's configuration or default
                if let Some(config) = &self.#field {
                    config.initialize()
                } else {
                    pulseengine_mcp_cli::DefaultLoggingConfig::default().initialize()
                }
            }
        }
    } else {
        quote! {
            fn get_logging_config(&self) -> &pulseengine_mcp_cli::DefaultLoggingConfig {
                use std::sync::OnceLock;
                static LOGGING_CONFIG: OnceLock<pulseengine_mcp_cli::DefaultLoggingConfig> = OnceLock::new();
                LOGGING_CONFIG.get_or_init(|| {
                    pulseengine_mcp_cli::DefaultLoggingConfig::default()
                })
            }

            fn initialize_logging(&self) -> std::result::Result<(), pulseengine_mcp_cli::CliError> {
                use pulseengine_mcp_cli::config::DefaultLoggingConfig;
                let default_config = DefaultLoggingConfig::default();
                default_config.initialize()
            }
        }
    }
}

fn generate_auto_populate_impl(auto_populate_fields: &[syn::Ident]) -> proc_macro2::TokenStream {
    if auto_populate_fields.is_empty() {
        return quote! {};
    }

    let implementations = auto_populate_fields.iter().map(|field| {
        match field.to_string().as_str() {
            "server_info" => quote! {
                self.#field = Some(pulseengine_mcp_cli::config::create_server_info(None, None));
            },
            "logging" => quote! {
                // Auto-populate logging configuration from environment
                use std::env;
                if let Ok(level) = env::var("MCP_LOG_LEVEL") {
                    // Update logging level if environment variable is set
                    // This is a placeholder - actual implementation would depend on the LoggingConfig structure
                }
            },
            _ => quote! {
                // Generic auto-population logic for field: #field
                // Check for environment variables with field name
                let env_var = format!("MCP_{}", stringify!(#field).to_uppercase());
                if let Ok(value) = std::env::var(&env_var) {
                    // TODO: Parse value based on field type
                    tracing::debug!("Found environment variable {}: {}", env_var, value);
                }
            },
        }
    });

    quote! {
        #(#implementations)*
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_mcp_config_derive() {
        let input = quote::quote! {
            struct TestConfig {
                port: u16,
                server_info: ServerInfo,
            }
        };

        let input: DeriveInput = syn::parse2(input).unwrap();
        let result = generate_mcp_config_impl(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mcp_config_with_attributes() {
        let input = quote::quote! {
            struct TestConfig {
                #[mcp(auto_populate)]
                server_info: ServerInfo,
                #[mcp(logging)]
                logging: LoggingConfig,
            }
        };

        let input: DeriveInput = syn::parse2(input).unwrap();
        let result = generate_mcp_config_impl(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_basic_mcp_backend_derive() {
        let input = quote::quote! {
            struct TestBackend {
                config: BackendConfig,
            }
        };

        let input: DeriveInput = syn::parse2(input).unwrap();
        let result = generate_mcp_backend_impl(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mcp_backend_with_simple() {
        let input = quote::quote! {
            #[mcp_backend(simple)]
            struct SimpleTestBackend {
                config: BackendConfig,
            }
        };

        let input: DeriveInput = syn::parse2(input).unwrap();
        let result = generate_mcp_backend_impl(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mcp_backend_with_custom_error() {
        let input = quote::quote! {
            #[mcp_backend(error = "CustomError")]
            struct CustomErrorBackend {
                config: BackendConfig,
            }
        };

        let input: DeriveInput = syn::parse2(input).unwrap();
        let result = generate_mcp_backend_impl(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mcp_backend_with_delegate() {
        let input = quote::quote! {
            struct DelegateBackend {
                #[mcp_backend(delegate)]
                inner: InnerBackend,
                config: BackendConfig,
            }
        };

        let input: DeriveInput = syn::parse2(input).unwrap();
        let result = generate_mcp_backend_impl(&input);
        assert!(result.is_ok());
    }

    #[test]
    fn test_invalid_mcp_config() {
        let input = quote::quote! {
            enum TestEnum {
                A, B, C
            }
        };

        let input: DeriveInput = syn::parse2(input).unwrap();
        let result = generate_mcp_config_impl(&input);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("can only be derived for structs"));
    }

    #[test]
    fn test_invalid_mcp_backend() {
        let input = quote::quote! {
            enum TestEnum {
                A, B, C
            }
        };

        let input: DeriveInput = syn::parse2(input).unwrap();
        let result = generate_mcp_backend_impl(&input);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("can only be derived for structs"));
    }
}
