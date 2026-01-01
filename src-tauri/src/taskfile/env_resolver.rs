#![allow(dead_code)]

use std::collections::HashMap;
use std::env;
use std::sync::OnceLock;

// SECURITY: Use static OnceLock to compile regexes once, avoiding potential panics on each call
// These patterns are valid and will compile successfully, but using OnceLock is safer and more efficient
static ENV_VAR_REGEX: OnceLock<regex_lite::Regex> = OnceLock::new();
static WORKFLOW_VAR_REGEX: OnceLock<regex_lite::Regex> = OnceLock::new();

fn get_env_var_regex() -> &'static regex_lite::Regex {
    ENV_VAR_REGEX.get_or_init(|| {
        regex_lite::Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)\}")
            .expect("ENV_VAR_REGEX pattern is invalid - this is a bug")
    })
}

fn get_workflow_var_regex() -> &'static regex_lite::Regex {
    WORKFLOW_VAR_REGEX.get_or_init(|| {
        regex_lite::Regex::new(r"\{\{([A-Za-z_][A-Za-z0-9_]*)\}\}")
            .expect("WORKFLOW_VAR_REGEX pattern is invalid - this is a bug")
    })
}

/// Resolves environment variable references in strings
/// Supports two patterns:
/// - ${VAR_NAME} - Direct environment variable reference (secrets)
/// - {{variable}} - Workflow variable reference (user input)
pub struct EnvResolver {
    /// Workflow variables (from user input or defaults)
    variables: HashMap<String, String>,
}

impl EnvResolver {
    pub fn new() -> Self {
        EnvResolver {
            variables: HashMap::new(),
        }
    }

    /// Create resolver with predefined workflow variables
    pub fn with_variables(variables: HashMap<String, String>) -> Self {
        EnvResolver { variables }
    }

    /// Set a workflow variable
    pub fn set_variable(&mut self, name: &str, value: &str) {
        self.variables.insert(name.to_string(), value.to_string());
    }

    /// Resolve all variable references in a string
    /// Returns the resolved string and a list of unresolved variables
    pub fn resolve(&self, input: &str) -> ResolveResult {
        let mut result = input.to_string();
        let mut unresolved = Vec::new();

        // Resolve ${ENV_VAR} patterns (environment variables)
        result = self.resolve_env_vars(&result, &mut unresolved);

        // Resolve {{variable}} patterns (workflow variables)
        result = self.resolve_workflow_vars(&result, &mut unresolved);

        ResolveResult {
            value: result,
            unresolved,
        }
    }

    /// Resolve only environment variable references
    fn resolve_env_vars(&self, input: &str, unresolved: &mut Vec<UnresolvedVar>) -> String {
        let re = get_env_var_regex();

        let mut result = input.to_string();
        for cap in re.captures_iter(input) {
            let full_match = &cap[0];
            let var_name = &cap[1];

            match env::var(var_name) {
                Ok(value) => {
                    result = result.replace(full_match, &value);
                }
                Err(_) => {
                    unresolved.push(UnresolvedVar {
                        name: var_name.to_string(),
                        var_type: VarType::Environment,
                    });
                }
            }
        }
        result
    }

    /// Resolve only workflow variable references
    fn resolve_workflow_vars(&self, input: &str, unresolved: &mut Vec<UnresolvedVar>) -> String {
        let re = get_workflow_var_regex();

        let mut result = input.to_string();
        for cap in re.captures_iter(input) {
            let full_match = &cap[0];
            let var_name = &cap[1];

            match self.variables.get(var_name) {
                Some(value) => {
                    result = result.replace(full_match, value);
                }
                None => {
                    unresolved.push(UnresolvedVar {
                        name: var_name.to_string(),
                        var_type: VarType::Workflow,
                    });
                }
            }
        }
        result
    }

    /// Check if a string contains any variable references
    pub fn has_references(input: &str) -> bool {
        input.contains("${") || input.contains("{{")
    }

    /// Extract all variable references from a string
    pub fn extract_references(input: &str) -> Vec<VarReference> {
        let mut refs = Vec::new();

        // Extract ${ENV_VAR} references
        for cap in get_env_var_regex().captures_iter(input) {
            refs.push(VarReference {
                name: cap[1].to_string(),
                var_type: VarType::Environment,
                original: cap[0].to_string(),
            });
        }

        // Extract {{variable}} references
        for cap in get_workflow_var_regex().captures_iter(input) {
            refs.push(VarReference {
                name: cap[1].to_string(),
                var_type: VarType::Workflow,
                original: cap[0].to_string(),
            });
        }

        refs
    }
}

impl Default for EnvResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VarType {
    Environment,
    Workflow,
}

#[derive(Debug, Clone)]
pub struct VarReference {
    pub name: String,
    pub var_type: VarType,
    pub original: String,
}

#[derive(Debug, Clone)]
pub struct UnresolvedVar {
    pub name: String,
    pub var_type: VarType,
}

#[derive(Debug, Clone)]
pub struct ResolveResult {
    pub value: String,
    pub unresolved: Vec<UnresolvedVar>,
}

impl ResolveResult {
    pub fn is_complete(&self) -> bool {
        self.unresolved.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_workflow_vars() {
        let mut resolver = EnvResolver::new();
        resolver.set_variable("username", "testuser");
        resolver.set_variable("query", "rust programming");

        let result = resolver.resolve("Hello, {{username}}! Search for {{query}}");
        assert_eq!(result.value, "Hello, testuser! Search for rust programming");
        assert!(result.is_complete());
    }

    #[test]
    fn test_resolve_env_vars() {
        env::set_var("TEST_VAR_123", "secret_value");
        let resolver = EnvResolver::new();

        let result = resolver.resolve("The secret is: ${TEST_VAR_123}");
        assert_eq!(result.value, "The secret is: secret_value");
        assert!(result.is_complete());
    }

    #[test]
    fn test_unresolved_vars() {
        let resolver = EnvResolver::new();
        let result = resolver.resolve("Hello, {{unknown}}!");

        assert_eq!(result.value, "Hello, {{unknown}}!");
        assert!(!result.is_complete());
        assert_eq!(result.unresolved.len(), 1);
        assert_eq!(result.unresolved[0].name, "unknown");
    }

    #[test]
    fn test_extract_references() {
        let refs = EnvResolver::extract_references("${API_KEY} and {{username}} plus ${SECRET}");
        assert_eq!(refs.len(), 3);
    }

    #[test]
    fn test_has_references() {
        assert!(EnvResolver::has_references("Hello ${WORLD}"));
        assert!(EnvResolver::has_references("Hello {{world}}"));
        assert!(!EnvResolver::has_references("Hello world"));
    }
}
