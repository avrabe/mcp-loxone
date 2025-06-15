//! Create API key for HTTP transport authentication

use clap::{Parser, ValueEnum};
use loxone_mcp_rust::http_transport::authentication::{AuthConfig, AuthManager, UserRole};

#[derive(Debug, Clone, ValueEnum)]
enum Role {
    Admin,
    Operator,
    ReadOnly,
    Limited,
    Monitor,
}

impl From<Role> for UserRole {
    fn from(role: Role) -> Self {
        match role {
            Role::Admin => UserRole::Admin,
            Role::Operator => UserRole::Operator,
            Role::ReadOnly => UserRole::ReadOnly,
            Role::Limited => UserRole::Limited,
            Role::Monitor => UserRole::Monitor,
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about = "Create API key for Loxone MCP server", long_about = None)]
struct Args {
    /// User role for the API key
    #[arg(short, long, value_enum, default_value = "operator")]
    role: Role,

    /// Description for the API key
    #[arg(short, long, default_value = "Default API key")]
    description: String,

    /// Validity period in days
    #[arg(short, long, default_value_t = 365)]
    validity_days: i64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Create auth manager with default config
    let auth_config = AuthConfig {
        key_rotation_days: args.validity_days,
        ..AuthConfig::default()
    };
    let auth_manager = AuthManager::new(auth_config);

    // Create API key
    let role = args.role.clone();
    let api_key = auth_manager
        .add_api_key(role.into(), args.description.clone())
        .await?;

    println!("\n🔑 API Key created successfully!");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("Role:        {:?}", args.role);
    println!("Description: {}", args.description);
    println!("Valid for:   {} days", args.validity_days);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\n⚠️  SAVE THIS KEY - IT CANNOT BE RETRIEVED LATER:");
    println!("\n{}\n", api_key);
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("\n📝 Usage examples:");
    println!(
        "  curl -H \"X-API-Key: {}\" http://localhost:3001/health",
        api_key
    );
    println!(
        "  curl -H \"Authorization: Bearer {}\" http://localhost:3001/health",
        api_key
    );
    println!("\n✅ API key is ready to use!");

    Ok(())
}
