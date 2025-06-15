//! Input sanitization and validation for production security

use crate::error::{LoxoneError, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Input sanitization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizationConfig {
    /// Enable sanitization
    pub enabled: bool,
    /// Maximum string length
    pub max_string_length: usize,
    /// Maximum array size
    pub max_array_size: usize,
    /// Maximum object depth
    pub max_object_depth: usize,
    /// Maximum number of object properties
    pub max_object_properties: usize,
    /// Enable HTML sanitization
    pub html_sanitization: bool,
    /// Enable SQL injection prevention
    pub sql_injection_prevention: bool,
    /// Enable XSS prevention
    pub xss_prevention: bool,
    /// Enable path traversal prevention
    pub path_traversal_prevention: bool,
    /// Custom sanitization rules
    pub custom_rules: Vec<SanitizationRule>,
    /// Whitelist patterns
    pub whitelist_patterns: HashMap<String, String>,
    /// Blacklist patterns
    pub blacklist_patterns: Vec<String>,
}

/// Custom sanitization rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SanitizationRule {
    /// Rule name
    pub name: String,
    /// Field path (e.g., "params.arguments.username")
    pub field_path: String,
    /// Rule type
    pub rule_type: SanitizationRuleType,
    /// Pattern or configuration
    pub pattern: String,
    /// Action to take when rule matches
    pub action: SanitizationAction,
}

/// Type of sanitization rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SanitizationRuleType {
    /// Regex pattern matching
    Regex,
    /// Length check
    Length,
    /// Character whitelist
    Whitelist,
    /// Character blacklist
    Blacklist,
    /// Custom validator function
    Custom,
}

/// Action to take when sanitization rule is triggered
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SanitizationAction {
    /// Remove matching content
    Remove,
    /// Replace with specified string
    Replace(String),
    /// Reject the entire request
    Reject,
    /// Log and continue
    Log,
    /// Encode/escape the content
    Encode,
}

/// Sanitization result
#[derive(Debug, Clone)]
pub struct SanitizationResult {
    /// Whether input is safe
    pub is_safe: bool,
    /// Sanitized data
    pub sanitized_data: Option<Value>,
    /// Issues found during sanitization
    pub issues: Vec<SanitizationIssue>,
    /// Warnings (non-blocking)
    pub warnings: Vec<String>,
}

/// Sanitization issue
#[derive(Debug, Clone)]
pub struct SanitizationIssue {
    /// Issue type
    pub issue_type: SanitizationIssueType,
    /// Field path where issue was found
    pub field_path: String,
    /// Description of the issue
    pub description: String,
    /// Severity level
    pub severity: SanitizationSeverity,
    /// Action taken
    pub action_taken: String,
}

/// Type of sanitization issue
#[derive(Debug, Clone)]
pub enum SanitizationIssueType {
    /// Potentially malicious content
    MaliciousContent,
    /// Input too long
    ExcessiveLength,
    /// Invalid characters
    InvalidCharacters,
    /// Suspicious patterns
    SuspiciousPattern,
    /// Path traversal attempt
    PathTraversal,
    /// SQL injection attempt
    SqlInjection,
    /// XSS attempt
    XssAttempt,
    /// Custom rule violation
    CustomRuleViolation,
}

/// Severity of sanitization issue
#[derive(Debug, Clone)]
pub enum SanitizationSeverity {
    /// Low severity - informational
    Low,
    /// Medium severity - suspicious
    Medium,
    /// High severity - likely attack
    High,
    /// Critical severity - definite attack
    Critical,
}

impl SanitizationConfig {
    /// Create strict sanitization configuration for production
    pub fn strict() -> Self {
        Self {
            enabled: true,
            max_string_length: 10000,
            max_array_size: 1000,
            max_object_depth: 10,
            max_object_properties: 100,
            html_sanitization: true,
            sql_injection_prevention: true,
            xss_prevention: true,
            path_traversal_prevention: true,
            custom_rules: Self::default_security_rules(),
            whitelist_patterns: Self::default_whitelist_patterns(),
            blacklist_patterns: Self::default_blacklist_patterns(),
        }
    }

    /// Create lenient sanitization configuration for development
    pub fn lenient() -> Self {
        Self {
            enabled: true,
            max_string_length: 100000,
            max_array_size: 10000,
            max_object_depth: 20,
            max_object_properties: 1000,
            html_sanitization: false,
            sql_injection_prevention: true,
            xss_prevention: true,
            path_traversal_prevention: true,
            custom_rules: Vec::new(),
            whitelist_patterns: HashMap::new(),
            blacklist_patterns: Self::basic_blacklist_patterns(),
        }
    }

    /// Create disabled sanitization configuration for testing
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            max_string_length: usize::MAX,
            max_array_size: usize::MAX,
            max_object_depth: usize::MAX,
            max_object_properties: usize::MAX,
            html_sanitization: false,
            sql_injection_prevention: false,
            xss_prevention: false,
            path_traversal_prevention: false,
            custom_rules: Vec::new(),
            whitelist_patterns: HashMap::new(),
            blacklist_patterns: Vec::new(),
        }
    }

    /// Default security rules
    fn default_security_rules() -> Vec<SanitizationRule> {
        vec![
            SanitizationRule {
                name: "username_validation".to_string(),
                field_path: "username".to_string(),
                rule_type: SanitizationRuleType::Regex,
                pattern: r"^[a-zA-Z0-9_-]+$".to_string(),
                action: SanitizationAction::Reject,
            },
            SanitizationRule {
                name: "email_validation".to_string(),
                field_path: "email".to_string(),
                rule_type: SanitizationRuleType::Regex,
                pattern: r"^[^\s@]+@[^\s@]+\.[^\s@]+$".to_string(),
                action: SanitizationAction::Reject,
            },
            SanitizationRule {
                name: "room_name_validation".to_string(),
                field_path: "room".to_string(),
                rule_type: SanitizationRuleType::Regex,
                pattern: r"^[a-zA-Z0-9\s_-]+$".to_string(),
                action: SanitizationAction::Reject,
            },
        ]
    }

    /// Default whitelist patterns for common fields
    fn default_whitelist_patterns() -> HashMap<String, String> {
        let mut patterns = HashMap::new();
        patterns.insert(
            "loxone_uuid".to_string(),
            r"^[0-9A-F]{8}-[0-9A-F]{6}-[0-9A-F]{3}$".to_string(),
        );
        patterns.insert(
            "device_action".to_string(),
            r"^(on|off|up|down|stop|pause|play|mute|unmute)$".to_string(),
        );
        patterns.insert(
            "room_name".to_string(),
            r"^[a-zA-Z0-9\s_-]{1,50}$".to_string(),
        );
        patterns
    }

    /// Default blacklist patterns for malicious content
    fn default_blacklist_patterns() -> Vec<String> {
        vec![
            // XSS patterns
            r"<script".to_string(),
            r"javascript:".to_string(),
            r"onload=".to_string(),
            r"onerror=".to_string(),
            r"onclick=".to_string(),
            // SQL injection patterns
            r"(?i)(union\s+select)".to_string(),
            r"(?i)(drop\s+table)".to_string(),
            r"(?i)(insert\s+into)".to_string(),
            r"(?i)(delete\s+from)".to_string(),
            r"(?i)(update\s+set)".to_string(),
            r"--".to_string(),
            r";--".to_string(),
            // Path traversal patterns
            r"\.\.\/".to_string(),
            r"\.\.\\".to_string(),
            r"%2e%2e%2f".to_string(),
            r"%2e%2e%5c".to_string(),
            // Command injection patterns
            r"(?i)(cmd\.exe)".to_string(),
            r"(?i)(powershell)".to_string(),
            r"(?i)(bash)".to_string(),
            r"(?i)(/bin/)".to_string(),
            r"\$\(.*\)".to_string(),
            r"`.*`".to_string(),
        ]
    }

    /// Basic blacklist patterns for development
    fn basic_blacklist_patterns() -> Vec<String> {
        vec![
            r"<script".to_string(),
            r"javascript:".to_string(),
            r"\.\.\/".to_string(),
            r"\.\.\\".to_string(),
        ]
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        if self.max_string_length == 0 {
            return Err(LoxoneError::invalid_input(
                "max_string_length must be greater than 0",
            ));
        }

        if self.max_array_size == 0 {
            return Err(LoxoneError::invalid_input(
                "max_array_size must be greater than 0",
            ));
        }

        if self.max_object_depth == 0 {
            return Err(LoxoneError::invalid_input(
                "max_object_depth must be greater than 0",
            ));
        }

        // Validate regex patterns
        for rule in &self.custom_rules {
            if matches!(rule.rule_type, SanitizationRuleType::Regex) {
                Regex::new(&rule.pattern).map_err(|e| {
                    LoxoneError::invalid_input(format!(
                        "Invalid regex pattern in rule '{}': {}",
                        rule.name, e
                    ))
                })?;
            }
        }

        for pattern in self.whitelist_patterns.values() {
            Regex::new(pattern).map_err(|e| {
                LoxoneError::invalid_input(format!("Invalid whitelist pattern: {}", e))
            })?;
        }

        for pattern in &self.blacklist_patterns {
            Regex::new(pattern).map_err(|e| {
                LoxoneError::invalid_input(format!("Invalid blacklist pattern: {}", e))
            })?;
        }

        Ok(())
    }

    /// Check if sanitization is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Input sanitizer
pub struct InputSanitizer {
    config: SanitizationConfig,
    blacklist_regexes: Vec<Regex>,
    whitelist_regexes: HashMap<String, Regex>,
    rule_regexes: HashMap<String, Regex>,
}

impl InputSanitizer {
    /// Create new input sanitizer
    pub fn new(config: SanitizationConfig) -> Result<Self> {
        config.validate()?;

        // Compile blacklist regexes
        let mut blacklist_regexes = Vec::new();
        for pattern in &config.blacklist_patterns {
            blacklist_regexes.push(Regex::new(pattern)?);
        }

        // Compile whitelist regexes
        let mut whitelist_regexes = HashMap::new();
        for (name, pattern) in &config.whitelist_patterns {
            whitelist_regexes.insert(name.clone(), Regex::new(pattern)?);
        }

        // Compile rule regexes
        let mut rule_regexes = HashMap::new();
        for rule in &config.custom_rules {
            if matches!(rule.rule_type, SanitizationRuleType::Regex) {
                rule_regexes.insert(rule.name.clone(), Regex::new(&rule.pattern)?);
            }
        }

        Ok(Self {
            config,
            blacklist_regexes,
            whitelist_regexes,
            rule_regexes,
        })
    }

    /// Sanitize input data
    pub fn sanitize(&self, data: &Value) -> SanitizationResult {
        if !self.config.enabled {
            return SanitizationResult {
                is_safe: true,
                sanitized_data: Some(data.clone()),
                issues: Vec::new(),
                warnings: Vec::new(),
            };
        }

        let mut issues = Vec::new();
        let mut warnings = Vec::new();
        let mut sanitized_data = data.clone();

        // Check overall structure limits
        self.check_structure_limits(data, "", &mut issues);

        // Sanitize content recursively
        self.sanitize_value(&mut sanitized_data, "", &mut issues, &mut warnings);

        let is_safe = issues.iter().all(|issue| {
            !matches!(
                issue.severity,
                SanitizationSeverity::High | SanitizationSeverity::Critical
            )
        });

        SanitizationResult {
            is_safe,
            sanitized_data: Some(sanitized_data),
            issues,
            warnings,
        }
    }

    /// Check structure limits (depth, size, etc.)
    fn check_structure_limits(
        &self,
        value: &Value,
        path: &str,
        issues: &mut Vec<SanitizationIssue>,
    ) {
        match value {
            Value::String(s) => {
                if s.len() > self.config.max_string_length {
                    issues.push(SanitizationIssue {
                        issue_type: SanitizationIssueType::ExcessiveLength,
                        field_path: path.to_string(),
                        description: format!(
                            "String length {} exceeds maximum {}",
                            s.len(),
                            self.config.max_string_length
                        ),
                        severity: SanitizationSeverity::Medium,
                        action_taken: "Truncated".to_string(),
                    });
                }
            }
            Value::Array(arr) => {
                if arr.len() > self.config.max_array_size {
                    issues.push(SanitizationIssue {
                        issue_type: SanitizationIssueType::ExcessiveLength,
                        field_path: path.to_string(),
                        description: format!(
                            "Array size {} exceeds maximum {}",
                            arr.len(),
                            self.config.max_array_size
                        ),
                        severity: SanitizationSeverity::Medium,
                        action_taken: "Truncated".to_string(),
                    });
                }

                for (i, item) in arr.iter().enumerate() {
                    self.check_structure_limits(item, &format!("{}[{}]", path, i), issues);
                }
            }
            Value::Object(obj) => {
                if obj.len() > self.config.max_object_properties {
                    issues.push(SanitizationIssue {
                        issue_type: SanitizationIssueType::ExcessiveLength,
                        field_path: path.to_string(),
                        description: format!(
                            "Object properties {} exceeds maximum {}",
                            obj.len(),
                            self.config.max_object_properties
                        ),
                        severity: SanitizationSeverity::Medium,
                        action_taken: "Limited".to_string(),
                    });
                }

                for (key, val) in obj {
                    let new_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };
                    self.check_structure_limits(val, &new_path, issues);
                }
            }
            _ => {}
        }
    }

    /// Sanitize a value recursively
    fn sanitize_value(
        &self,
        value: &mut Value,
        path: &str,
        issues: &mut Vec<SanitizationIssue>,
        warnings: &mut Vec<String>,
    ) {
        match value {
            Value::String(ref mut s) => {
                self.sanitize_string(s, path, issues, warnings);
            }
            Value::Array(ref mut arr) => {
                // Truncate array if too large
                if arr.len() > self.config.max_array_size {
                    arr.truncate(self.config.max_array_size);
                }

                for (i, item) in arr.iter_mut().enumerate() {
                    self.sanitize_value(item, &format!("{}[{}]", path, i), issues, warnings);
                }
            }
            Value::Object(ref mut obj) => {
                // Limit object properties if too many
                if obj.len() > self.config.max_object_properties {
                    let keys_to_remove: Vec<_> = obj
                        .keys()
                        .skip(self.config.max_object_properties)
                        .cloned()
                        .collect();
                    for key in keys_to_remove {
                        obj.remove(&key);
                    }
                }

                for (key, val) in obj.iter_mut() {
                    let new_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };
                    self.sanitize_value(val, &new_path, issues, warnings);
                }
            }
            _ => {}
        }
    }

    /// Sanitize a string value
    fn sanitize_string(
        &self,
        s: &mut String,
        path: &str,
        issues: &mut Vec<SanitizationIssue>,
        warnings: &mut Vec<String>,
    ) {
        let original = s.clone();

        // Truncate if too long
        if s.len() > self.config.max_string_length {
            s.truncate(self.config.max_string_length);
        }

        // Check blacklist patterns
        for regex in &self.blacklist_regexes {
            if regex.is_match(s) {
                issues.push(SanitizationIssue {
                    issue_type: SanitizationIssueType::MaliciousContent,
                    field_path: path.to_string(),
                    description: "Content matches blacklist pattern".to_string(),
                    severity: SanitizationSeverity::High,
                    action_taken: "Content sanitized".to_string(),
                });

                // Remove or replace malicious content
                *s = regex.replace_all(s, "").to_string();
            }
        }

        // Apply custom rules
        for rule in &self.config.custom_rules {
            if rule.field_path == path || path.ends_with(&rule.field_path) {
                self.apply_custom_rule(s, rule, path, issues, warnings);
            }
        }

        // XSS prevention
        if self.config.xss_prevention {
            self.prevent_xss(s, path, issues);
        }

        // HTML sanitization
        if self.config.html_sanitization {
            self.sanitize_html(s, path, issues);
        }

        // SQL injection prevention
        if self.config.sql_injection_prevention {
            self.prevent_sql_injection(s, path, issues);
        }

        // Path traversal prevention
        if self.config.path_traversal_prevention {
            self.prevent_path_traversal(s, path, issues);
        }

        if *s != original {
            warnings.push(format!("Content modified in field: {}", path));
        }
    }

    /// Apply custom sanitization rule
    fn apply_custom_rule(
        &self,
        s: &mut String,
        rule: &SanitizationRule,
        path: &str,
        issues: &mut Vec<SanitizationIssue>,
        _warnings: &mut Vec<String>,
    ) {
        match rule.rule_type {
            SanitizationRuleType::Regex => {
                if let Some(regex) = self.rule_regexes.get(&rule.name) {
                    if !regex.is_match(s) {
                        issues.push(SanitizationIssue {
                            issue_type: SanitizationIssueType::CustomRuleViolation,
                            field_path: path.to_string(),
                            description: format!(
                                "Field does not match required pattern: {}",
                                rule.name
                            ),
                            severity: SanitizationSeverity::Medium,
                            action_taken: match &rule.action {
                                SanitizationAction::Reject => "Request rejected".to_string(),
                                SanitizationAction::Remove => "Content removed".to_string(),
                                SanitizationAction::Replace(replacement) => {
                                    format!("Replaced with: {}", replacement)
                                }
                                SanitizationAction::Log => "Logged violation".to_string(),
                                SanitizationAction::Encode => "Content encoded".to_string(),
                            },
                        });

                        match &rule.action {
                            SanitizationAction::Remove => s.clear(),
                            SanitizationAction::Replace(replacement) => *s = replacement.clone(),
                            SanitizationAction::Encode => {
                                // Simple HTML encoding
                                *s = s
                                    .replace("&", "&amp;")
                                    .replace("<", "&lt;")
                                    .replace(">", "&gt;")
                                    .replace("\"", "&quot;")
                                    .replace("'", "&#39;");
                            }
                            _ => {}
                        }
                    }
                }
            }
            SanitizationRuleType::Length => {
                if let Ok(max_len) = rule.pattern.parse::<usize>() {
                    if s.len() > max_len {
                        s.truncate(max_len);
                        issues.push(SanitizationIssue {
                            issue_type: SanitizationIssueType::ExcessiveLength,
                            field_path: path.to_string(),
                            description: format!("Field length exceeded maximum: {}", max_len),
                            severity: SanitizationSeverity::Low,
                            action_taken: "Truncated".to_string(),
                        });
                    }
                }
            }
            _ => {
                // Other rule types would be implemented here
            }
        }
    }

    /// Prevent XSS attacks
    fn prevent_xss(&self, s: &mut String, path: &str, issues: &mut Vec<SanitizationIssue>) {
        let xss_patterns = [
            r"<script",
            r"javascript:",
            r"onload=",
            r"onerror=",
            r"onclick=",
            r"onmouseover=",
            r"onfocus=",
            r"onblur=",
        ];

        for pattern in &xss_patterns {
            if let Ok(regex) = Regex::new(&format!("(?i){}", pattern)) {
                if regex.is_match(s) {
                    issues.push(SanitizationIssue {
                        issue_type: SanitizationIssueType::XssAttempt,
                        field_path: path.to_string(),
                        description: format!("Potential XSS attempt detected: {}", pattern),
                        severity: SanitizationSeverity::High,
                        action_taken: "Content sanitized".to_string(),
                    });

                    *s = regex.replace_all(s, "").to_string();
                }
            }
        }
    }

    /// Sanitize HTML content
    fn sanitize_html(&self, s: &mut String, path: &str, issues: &mut Vec<SanitizationIssue>) {
        // Simple HTML tag removal - in production use a proper HTML sanitizer
        if let Ok(regex) = Regex::new(r"<[^>]*>") {
            if regex.is_match(s) {
                issues.push(SanitizationIssue {
                    issue_type: SanitizationIssueType::MaliciousContent,
                    field_path: path.to_string(),
                    description: "HTML tags detected and removed".to_string(),
                    severity: SanitizationSeverity::Medium,
                    action_taken: "HTML tags removed".to_string(),
                });

                *s = regex.replace_all(s, "").to_string();
            }
        }
    }

    /// Prevent SQL injection
    fn prevent_sql_injection(
        &self,
        s: &mut String,
        path: &str,
        issues: &mut Vec<SanitizationIssue>,
    ) {
        let sql_patterns = [
            r"(?i)(union\s+select)",
            r"(?i)(drop\s+table)",
            r"(?i)(insert\s+into)",
            r"(?i)(delete\s+from)",
            r"(?i)(update\s+set)",
            r"--",
            r";--",
        ];

        for pattern in &sql_patterns {
            if let Ok(regex) = Regex::new(pattern) {
                if regex.is_match(s) {
                    issues.push(SanitizationIssue {
                        issue_type: SanitizationIssueType::SqlInjection,
                        field_path: path.to_string(),
                        description: format!("Potential SQL injection detected: {}", pattern),
                        severity: SanitizationSeverity::Critical,
                        action_taken: "Content sanitized".to_string(),
                    });

                    *s = regex.replace_all(s, "").to_string();
                }
            }
        }
    }

    /// Prevent path traversal attacks
    fn prevent_path_traversal(
        &self,
        s: &mut String,
        path: &str,
        issues: &mut Vec<SanitizationIssue>,
    ) {
        let traversal_patterns = [r"\.\.\/", r"\.\.\\", r"%2e%2e%2f", r"%2e%2e%5c"];

        for pattern in &traversal_patterns {
            if let Ok(regex) = Regex::new(&format!("(?i){}", pattern)) {
                if regex.is_match(s) {
                    issues.push(SanitizationIssue {
                        issue_type: SanitizationIssueType::PathTraversal,
                        field_path: path.to_string(),
                        description: format!("Path traversal attempt detected: {}", pattern),
                        severity: SanitizationSeverity::High,
                        action_taken: "Content sanitized".to_string(),
                    });

                    *s = regex.replace_all(s, "").to_string();
                }
            }
        }
    }

    /// Get configuration
    pub fn get_config(&self) -> &SanitizationConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_sanitization_config() {
        let config = SanitizationConfig::strict();
        assert!(config.validate().is_ok());
        assert!(config.is_enabled());
    }

    #[test]
    fn test_input_sanitizer_creation() {
        let config = SanitizationConfig::strict();
        let sanitizer = InputSanitizer::new(config);
        assert!(sanitizer.is_ok());
    }

    #[test]
    fn test_xss_prevention() {
        let config = SanitizationConfig::strict();
        let sanitizer = InputSanitizer::new(config).unwrap();

        let malicious_data = json!({
            "message": "<script>alert('xss')</script>Hello"
        });

        let result = sanitizer.sanitize(&malicious_data);
        assert!(!result.issues.is_empty());
        assert!(result
            .issues
            .iter()
            .any(|issue| matches!(issue.issue_type, SanitizationIssueType::XssAttempt)));
    }

    #[test]
    fn test_sql_injection_prevention() {
        let config = SanitizationConfig::strict();
        let sanitizer = InputSanitizer::new(config).unwrap();

        let malicious_data = json!({
            "query": "SELECT * FROM users; DROP TABLE users; --"
        });

        let result = sanitizer.sanitize(&malicious_data);
        assert!(!result.issues.is_empty());
        assert!(result
            .issues
            .iter()
            .any(|issue| matches!(issue.issue_type, SanitizationIssueType::SqlInjection)));
    }

    #[test]
    fn test_path_traversal_prevention() {
        let config = SanitizationConfig::strict();
        let sanitizer = InputSanitizer::new(config).unwrap();

        let malicious_data = json!({
            "file": "../../../etc/passwd"
        });

        let result = sanitizer.sanitize(&malicious_data);
        assert!(!result.issues.is_empty());
        assert!(result
            .issues
            .iter()
            .any(|issue| matches!(issue.issue_type, SanitizationIssueType::PathTraversal)));
    }

    #[test]
    fn test_structure_limits() {
        let config = SanitizationConfig::strict();
        let sanitizer = InputSanitizer::new(config).unwrap();

        // Test string length limit
        let long_string = "x".repeat(20000);
        let data = json!({ "message": long_string });

        let result = sanitizer.sanitize(&data);
        assert!(!result.issues.is_empty());
        assert!(result
            .issues
            .iter()
            .any(|issue| matches!(issue.issue_type, SanitizationIssueType::ExcessiveLength)));
    }

    #[test]
    fn test_clean_input() {
        let config = SanitizationConfig::strict();
        let sanitizer = InputSanitizer::new(config).unwrap();

        let clean_data = json!({
            "room": "Kitchen",
            "action": "on",
            "device": "12345678-ABCDEF-123"
        });

        let result = sanitizer.sanitize(&clean_data);
        assert!(result.is_safe);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_disabled_sanitization() {
        let config = SanitizationConfig::disabled();
        let sanitizer = InputSanitizer::new(config).unwrap();

        let malicious_data = json!({
            "message": "<script>alert('xss')</script>",
            "query": "DROP TABLE users;"
        });

        let result = sanitizer.sanitize(&malicious_data);
        assert!(result.is_safe); // Should be safe when disabled
        assert!(result.issues.is_empty());
    }
}
