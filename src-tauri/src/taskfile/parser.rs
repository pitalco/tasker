#![allow(dead_code)]

use super::models::{Taskfile, ValidationError, ValidationResult};
use std::path::Path;

/// Parse a Taskfile from YAML string
pub fn parse_yaml(yaml_content: &str) -> Result<Taskfile, String> {
    serde_yaml::from_str(yaml_content).map_err(|e| format!("Failed to parse Taskfile YAML: {}", e))
}

/// Parse a Taskfile from a file path
pub fn parse_file(path: &Path) -> Result<Taskfile, String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))?;
    parse_yaml(&content)
}

/// Validate a parsed Taskfile
pub fn validate(taskfile: &Taskfile) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Validate schema version
    if taskfile.taskfile != "1.0" {
        warnings.push(format!(
            "Unknown taskfile version '{}', expected '1.0'",
            taskfile.taskfile
        ));
    }

    // Validate metadata
    if taskfile.metadata.name.is_empty() {
        errors.push(ValidationError {
            path: "metadata.name".to_string(),
            message: "Workflow name is required".to_string(),
        });
    }

    // Validate steps
    if taskfile.steps.is_empty() {
        errors.push(ValidationError {
            path: "steps".to_string(),
            message: "At least one step is required".to_string(),
        });
    }

    // Check for duplicate step IDs
    let mut step_ids = std::collections::HashSet::new();
    for (i, step) in taskfile.steps.iter().enumerate() {
        if step.id.is_empty() {
            errors.push(ValidationError {
                path: format!("steps[{}].id", i),
                message: "Step ID is required".to_string(),
            });
        } else if !step_ids.insert(&step.id) {
            errors.push(ValidationError {
                path: format!("steps[{}].id", i),
                message: format!("Duplicate step ID: '{}'", step.id),
            });
        }
    }

    // Check for duplicate variable names
    let mut var_names = std::collections::HashSet::new();
    for (i, var) in taskfile.variables.iter().enumerate() {
        if var.name.is_empty() {
            errors.push(ValidationError {
                path: format!("variables[{}].name", i),
                message: "Variable name is required".to_string(),
            });
        } else if !var_names.insert(&var.name) {
            errors.push(ValidationError {
                path: format!("variables[{}].name", i),
                message: format!("Duplicate variable name: '{}'", var.name),
            });
        }
    }

    // Validate env dependencies
    for (i, env) in taskfile.dependencies.env.iter().enumerate() {
        if env.name.is_empty() {
            errors.push(ValidationError {
                path: format!("dependencies.env[{}].name", i),
                message: "Environment variable name is required".to_string(),
            });
        }
    }

    // Validate cron expression format (basic check)
    if let Some(cron) = &taskfile.triggers.cron {
        if cron.enabled && cron.expression.split_whitespace().count() != 5 {
            warnings.push(format!(
                "Cron expression '{}' may be invalid (expected 5 fields)",
                cron.expression
            ));
        }
    }

    // Validate HTTP trigger
    if let Some(http) = &taskfile.triggers.http {
        if http.enabled && !http.path.starts_with('/') {
            errors.push(ValidationError {
                path: "triggers.http.path".to_string(),
                message: "HTTP path must start with '/'".to_string(),
            });
        }
    }

    // Validate output variables exist in extract steps
    for output_var in &taskfile.output.variables {
        let extracted = taskfile.steps.iter().any(|step| {
            matches!(&step.action, super::models::TaskfileAction::Extract { variable, .. } if variable == output_var)
        });
        let defined = taskfile.variables.iter().any(|v| &v.name == output_var);

        if !extracted && !defined {
            warnings.push(format!(
                "Output variable '{}' is not extracted by any step or defined as input",
                output_var
            ));
        }
    }

    ValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_taskfile() {
        let yaml = r#"
taskfile: "1.0"
metadata:
  name: "Test Workflow"
steps:
  - id: "navigate"
    action:
      type: "navigate"
      url: "https://example.com"
"#;
        let result = parse_yaml(yaml);
        assert!(result.is_ok());
        let taskfile = result.unwrap();
        assert_eq!(taskfile.metadata.name, "Test Workflow");
        assert_eq!(taskfile.steps.len(), 1);
    }

    #[test]
    fn test_validate_empty_name() {
        let yaml = r#"
taskfile: "1.0"
metadata:
  name: ""
steps:
  - id: "test"
    action:
      type: "navigate"
      url: "https://example.com"
"#;
        let taskfile = parse_yaml(yaml).unwrap();
        let result = validate(&taskfile);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.path == "metadata.name"));
    }
}
