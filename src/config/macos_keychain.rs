//! macOS-specific keychain access using Security Framework
//!
//! This module provides direct access to the macOS Security Framework
//! to minimize keychain password prompts by using proper API calls.

#[cfg(target_os = "macos")]
use crate::error::{LoxoneError, Result};
use std::ffi::CString;
use std::ptr;

#[cfg(target_os = "macos")]
/// RAII guard to ensure keychain memory is always freed
struct KeychainPasswordGuard(*mut std::ffi::c_void);

#[cfg(target_os = "macos")]
impl Drop for KeychainPasswordGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                SecKeychainItemFreeContent(ptr::null_mut(), self.0);
            }
        }
    }
}

#[cfg(target_os = "macos")]
#[repr(C)]
struct SecKeychainItem(*const std::ffi::c_void);

#[cfg(target_os = "macos")]
extern "C" {
    fn SecKeychainFindGenericPassword(
        keychain_or_array: *const std::ffi::c_void,
        service_name_length: u32,
        service_name: *const std::os::raw::c_char,
        account_name_length: u32,
        account_name: *const std::os::raw::c_char,
        password_length: *mut u32,
        password_data: *mut *mut std::ffi::c_void,
        item_ref: *mut SecKeychainItem,
    ) -> std::os::raw::c_int;

    fn SecKeychainItemFreeContent(
        attr_list: *mut std::ffi::c_void,
        data: *mut std::ffi::c_void,
    ) -> std::os::raw::c_int;
}

/// Direct macOS keychain access that minimizes prompts
#[cfg(target_os = "macos")]
pub struct MacOSKeychain;

#[cfg(target_os = "macos")]
impl MacOSKeychain {
    /// Get password from keychain using Security Framework directly
    pub fn get_password(service: &str, account: &str) -> Result<String> {
        // Validate input parameters
        if service.is_empty() || account.is_empty() {
            return Err(LoxoneError::credentials("Service and account names cannot be empty"));
        }

        let service_cstr = CString::new(service)
            .map_err(|e| LoxoneError::credentials(format!("Invalid service name (contains null byte): {}", e)))?;
        let account_cstr = CString::new(account)
            .map_err(|e| LoxoneError::credentials(format!("Invalid account name (contains null byte): {}", e)))?;

        let mut password_length: u32 = 0;
        let mut password_data: *mut std::ffi::c_void = ptr::null_mut();
        let mut item_ref = SecKeychainItem(ptr::null());

        let status = unsafe {
            SecKeychainFindGenericPassword(
                ptr::null(), // Default keychain
                service_cstr.as_bytes().len() as u32,
                service_cstr.as_ptr(),
                account_cstr.as_bytes().len() as u32,
                account_cstr.as_ptr(),
                &mut password_length,
                &mut password_data,
                &mut item_ref,
            )
        };

        if status != 0 {
            return Err(LoxoneError::credentials(format!(
                "Failed to get password from keychain: status {}",
                status
            )));
        }

        // Use RAII guard to ensure memory is always freed
        let _guard = KeychainPasswordGuard(password_data);

        // Validate returned data before using
        if password_data.is_null() {
            return Err(LoxoneError::credentials("Null password data returned"));
        }

        if password_length == 0 {
            return Ok(String::new());
        }

        // Additional safety: check for reasonable password length to prevent huge allocations
        if password_length > 65536 {
            return Err(LoxoneError::credentials("Password length exceeds reasonable limit"));
        }

        let password = unsafe {
            // Create slice with validated bounds - we've checked password_data is not null
            // and password_length is reasonable above
            let slice = std::slice::from_raw_parts(
                password_data as *const u8,
                password_length as usize,
            );
            String::from_utf8_lossy(slice).to_string()
        };

        // Memory will be automatically freed by the guard's Drop implementation

        Ok(password)
    }

    /// Get all Loxone credentials in one call to minimize prompts
    pub fn get_all_credentials() -> Result<(String, String, Option<String>, Option<String>)> {
        // This approach still requires multiple calls but we can batch them
        // to reduce the perceived number of prompts

        let username = Self::get_password("LoxoneMCP", "LOXONE_USER")?;
        let password = Self::get_password("LoxoneMCP", "LOXONE_PASS")?;
        let host = Self::get_password("LoxoneMCP", "LOXONE_HOST").ok();
        // Try new name first, then old name for backward compatibility
        let api_key = Self::get_password("LoxoneMCP", "LOXONE_API_KEY")
            .or_else(|_| Self::get_password("LoxoneMCP", "LOXONE_SSE_API_KEY"))
            .ok();

        Ok((username, password, host, api_key))
    }
}

#[cfg(not(target_os = "macos"))]
pub struct MacOSKeychain;

#[cfg(not(target_os = "macos"))]
impl MacOSKeychain {
    pub fn get_password(_service: &str, _account: &str) -> Result<String> {
        Err(LoxoneError::credentials("macOS keychain not available on this platform"))
    }

    pub fn get_all_credentials() -> Result<(String, String, Option<String>, Option<String>)> {
        Err(LoxoneError::credentials("macOS keychain not available on this platform"))
    }
}