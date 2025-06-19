//! SSH-style security for credential storage
//!
//! This module implements SSH-like permission checking and enforcement
//! for secure storage of API keys and credentials, following the same
//! security principles as SSH for protecting sensitive data.

use crate::error::{LoxoneError, Result};
use std::fs;
use std::path::Path;
use tracing::{debug, info, warn};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// SSH-style permission constants
pub mod permissions {
    /// Directory should be accessible only by owner (like ~/.ssh)
    pub const SECURE_DIR: u32 = 0o700;
    /// Private files should be readable/writable only by owner (like private keys)
    pub const SECURE_FILE: u32 = 0o600;
    /// Public files can be readable by others (like public keys)
    pub const PUBLIC_FILE: u32 = 0o644;
}

/// Security validation result
#[derive(Debug)]
pub enum SecurityCheck {
    /// File/directory has secure permissions
    Secure,
    /// File/directory has insecure permissions
    Insecure {
        /// Current permissions (octal)
        current: u32,
        /// Required permissions (octal)
        required: u32,
        /// Path being checked
        path: String,
        /// Suggested fix command
        fix_command: String,
    },
    /// Could not check permissions (e.g., on Windows)
    Unchecked {
        /// Reason why check was skipped
        reason: String,
    },
}

/// Check if a directory has secure SSH-style permissions
pub fn check_secure_directory(dir_path: &Path) -> Result<SecurityCheck> {
    if !dir_path.exists() {
        return Ok(SecurityCheck::Unchecked {
            reason: "Directory does not exist yet".to_string(),
        });
    }

    let metadata = fs::metadata(dir_path)
        .map_err(|e| LoxoneError::config(format!("Cannot read directory metadata: {}", e)))?;

    if !metadata.is_dir() {
        return Err(LoxoneError::config("Path is not a directory"));
    }

    #[cfg(unix)]
    {
        let mode = metadata.permissions().mode();
        let dir_perms = mode & 0o777;

        // Check if directory is accessible by group or others (should be 700)
        if dir_perms & 0o077 != 0 {
            return Ok(SecurityCheck::Insecure {
                current: dir_perms,
                required: permissions::SECURE_DIR,
                path: dir_path.display().to_string(),
                fix_command: format!("chmod 700 {}", dir_path.display()),
            });
        }

        debug!("Directory {} has secure permissions: {:o}", dir_path.display(), dir_perms);
        Ok(SecurityCheck::Secure)
    }

    #[cfg(windows)]
    {
        // On Windows, we do basic checks since ACL checking requires external crates
        if metadata.permissions().readonly() {
            return Err(LoxoneError::config(
                "Directory is read-only, cannot be used for credentials"
            ));
        }

        // Windows doesn't have Unix-style permissions, so we skip detailed checking
        warn!("Windows detected: Cannot perform detailed permission checking. Ensure only your user account has access to {}", dir_path.display());
        Ok(SecurityCheck::Unchecked {
            reason: "Windows: Cannot check Unix-style permissions".to_string(),
        })
    }
}

/// Check if a file has secure SSH-style permissions
pub fn check_secure_file(file_path: &Path) -> Result<SecurityCheck> {
    if !file_path.exists() {
        return Ok(SecurityCheck::Unchecked {
            reason: "File does not exist yet".to_string(),
        });
    }

    let metadata = fs::metadata(file_path)
        .map_err(|e| LoxoneError::config(format!("Cannot read file metadata: {}", e)))?;

    #[cfg(unix)]
    {
        let mode = metadata.permissions().mode();
        let file_perms = mode & 0o777;

        // Private key files should be 600 (owner read/write only)
        if file_perms & 0o077 != 0 {
            return Ok(SecurityCheck::Insecure {
                current: file_perms,
                required: permissions::SECURE_FILE,
                path: file_path.display().to_string(),
                fix_command: format!("chmod 600 {}", file_path.display()),
            });
        }

        debug!("File {} has secure permissions: {:o}", file_path.display(), file_perms);
        Ok(SecurityCheck::Secure)
    }

    #[cfg(windows)]
    {
        // Basic check - ensure file is not read-only if we need to write to it
        if metadata.permissions().readonly() {
            warn!("File {} is read-only, this may cause issues", file_path.display());
        }

        // Windows doesn't have Unix-style permissions
        warn!("Windows detected: Cannot perform detailed permission checking. Ensure only your user account has access to {}", file_path.display());
        Ok(SecurityCheck::Unchecked {
            reason: "Windows: Cannot check Unix-style permissions".to_string(),
        })
    }
}

/// Create a directory with secure SSH-style permissions
pub fn create_secure_directory(dir_path: &Path) -> Result<()> {
    // Create directory if it doesn't exist
    fs::create_dir_all(dir_path)
        .map_err(|e| LoxoneError::config(format!("Failed to create directory: {}", e)))?;

    #[cfg(unix)]
    {
        // Set directory permissions to 700 (owner only)
        let mut perms = fs::metadata(dir_path)
            .map_err(|e| LoxoneError::config(format!("Cannot read directory metadata: {}", e)))?
            .permissions();
        perms.set_mode(permissions::SECURE_DIR);
        fs::set_permissions(dir_path, perms)
            .map_err(|e| LoxoneError::config(format!("Failed to set directory permissions: {}", e)))?;
        
        info!("Created secure directory {} with permissions 700", dir_path.display());
    }

    #[cfg(windows)]
    {
        info!("Created directory {} (Windows: manual permission setup recommended)", dir_path.display());
    }

    Ok(())
}

/// Create a file with secure SSH-style permissions
pub fn create_secure_file(file_path: &Path) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = file_path.parent() {
        create_secure_directory(parent)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        // Create file with secure permissions from the start
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .mode(permissions::SECURE_FILE) // Owner read/write only
            .open(file_path)
            .map_err(|e| LoxoneError::config(format!("Failed to create secure file: {}", e)))?;
        
        info!("Created secure file {} with permissions 600", file_path.display());
    }

    #[cfg(windows)]
    {
        // On Windows, create the file normally
        fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(file_path)
            .map_err(|e| LoxoneError::config(format!("Failed to create file: {}", e)))?;

        // Make file not read-only
        let mut perms = fs::metadata(file_path)
            .map_err(|e| LoxoneError::config(format!("Cannot read file metadata: {}", e)))?
            .permissions();
        perms.set_readonly(false);
        fs::set_permissions(file_path, perms)
            .map_err(|e| LoxoneError::config(format!("Failed to set file permissions: {}", e)))?;
        
        info!("Created file {} (Windows: manual permission setup recommended)", file_path.display());
    }

    Ok(())
}

/// Write to a file while ensuring secure permissions (atomic write)
pub async fn write_secure_file(file_path: &Path, content: &str) -> Result<()> {
    use tokio::fs;
    
    // Ensure parent directory exists with secure permissions
    if let Some(parent) = file_path.parent() {
        create_secure_directory(parent)?;
    }
    
    // Create temporary file with secure permissions
    let temp_file = file_path.with_extension("tmp");
    create_secure_file(&temp_file)?;
    
    // Write content to temp file
    fs::write(&temp_file, content).await
        .map_err(|e| LoxoneError::config(format!("Failed to write secure file: {}", e)))?;
    
    // Atomic rename (preserves permissions)
    fs::rename(&temp_file, file_path).await
        .map_err(|e| LoxoneError::config(format!("Failed to rename secure file: {}", e)))?;
    
    debug!("Securely wrote to file: {}", file_path.display());
    Ok(())
}

/// Fix permissions on an existing directory to be secure
pub fn fix_directory_permissions(dir_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(dir_path)
            .map_err(|e| LoxoneError::config(format!("Cannot read directory metadata: {}", e)))?
            .permissions();
        perms.set_mode(permissions::SECURE_DIR);
        fs::set_permissions(dir_path, perms)
            .map_err(|e| LoxoneError::config(format!("Failed to fix directory permissions: {}", e)))?;
        
        info!("Fixed directory permissions for {} to 700", dir_path.display());
        Ok(())
    }

    #[cfg(windows)]
    {
        warn!("Windows detected: Cannot automatically fix permissions for {}. Please ensure only your user account has access.", dir_path.display());
        Ok(())
    }
}

/// Fix permissions on an existing file to be secure
pub fn fix_file_permissions(file_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(file_path)
            .map_err(|e| LoxoneError::config(format!("Cannot read file metadata: {}", e)))?
            .permissions();
        perms.set_mode(permissions::SECURE_FILE);
        fs::set_permissions(file_path, perms)
            .map_err(|e| LoxoneError::config(format!("Failed to fix file permissions: {}", e)))?;
        
        info!("Fixed file permissions for {} to 600", file_path.display());
        Ok(())
    }

    #[cfg(windows)]
    {
        warn!("Windows detected: Cannot automatically fix permissions for {}. Please ensure only your user account has access.", file_path.display());
        Ok(())
    }
}

/// Validate all security requirements for credential storage
pub fn validate_credential_security(dir_path: &Path, file_paths: &[&Path]) -> Result<Vec<SecurityCheck>> {
    let mut results = Vec::new();

    // Check directory permissions
    results.push(check_secure_directory(dir_path)?);

    // Check each file's permissions
    for file_path in file_paths {
        results.push(check_secure_file(file_path)?);
    }

    Ok(results)
}

/// Print SSH-style security warning messages
pub fn print_security_warnings(checks: &[SecurityCheck]) {
    for check in checks {
        match check {
            SecurityCheck::Insecure { current, required: _, path, fix_command } => {
                eprintln!("⚠️  SECURITY WARNING:");
                eprintln!("Permissions {:o} for '{}' are too open.", current, path);
                eprintln!("It is recommended that your credential files are NOT accessible by others.");
                eprintln!("Run: {}", fix_command);
                eprintln!();
            }
            SecurityCheck::Unchecked { reason } => {
                debug!("Security check skipped: {}", reason);
            }
            SecurityCheck::Secure => {
                debug!("Security check passed");
            }
        }
    }
}

/// Auto-fix insecure permissions with user consent
pub fn auto_fix_permissions(checks: &[SecurityCheck], auto_fix: bool) -> Result<()> {
    let insecure_items: Vec<_> = checks.iter()
        .filter_map(|check| match check {
            SecurityCheck::Insecure { path, .. } => Some(path),
            _ => None,
        })
        .collect();

    if insecure_items.is_empty() {
        return Ok(());
    }

    if auto_fix {
        info!("Auto-fixing {} insecure permissions...", insecure_items.len());
        
        for check in checks {
            if let SecurityCheck::Insecure { path, .. } = check {
                let path = Path::new(path);
                if path.is_dir() {
                    fix_directory_permissions(path)?;
                } else {
                    fix_file_permissions(path)?;
                }
            }
        }
        
        info!("✅ All permissions have been fixed");
    } else {
        warn!("❌ Found {} items with insecure permissions. Use --auto-fix to correct them automatically.", insecure_items.len());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_create_secure_directory() {
        let temp_dir = TempDir::new().unwrap();
        let secure_dir = temp_dir.path().join("test_secure");

        create_secure_directory(&secure_dir).unwrap();
        assert!(secure_dir.exists());
        assert!(secure_dir.is_dir());

        // On Unix systems, check permissions
        #[cfg(unix)]
        {
            let check = check_secure_directory(&secure_dir).unwrap();
            assert!(matches!(check, SecurityCheck::Secure));
        }
    }

    #[test]
    fn test_create_secure_file() {
        let temp_dir = TempDir::new().unwrap();
        let secure_file = temp_dir.path().join("test_secure_file");

        create_secure_file(&secure_file).unwrap();
        assert!(secure_file.exists());
        assert!(secure_file.is_file());

        // On Unix systems, check permissions
        #[cfg(unix)]
        {
            let check = check_secure_file(&secure_file).unwrap();
            assert!(matches!(check, SecurityCheck::Secure));
        }
    }
}