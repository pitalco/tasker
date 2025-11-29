use serde::{Deserialize, Serialize};

/// Root Taskfile structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Taskfile {
    /// Schema version (e.g., "1.0")
    pub taskfile: String,

    /// Workflow metadata
    pub metadata: TaskfileMetadata,

    /// Trigger configurations for execution
    #[serde(default)]
    pub triggers: Triggers,

    /// External dependencies
    #[serde(default)]
    pub dependencies: Dependencies,

    /// Resource limits
    #[serde(default)]
    pub limits: Limits,

    /// Input variables
    #[serde(default)]
    pub variables: Vec<Variable>,

    /// Execution configuration
    #[serde(default)]
    pub execution: ExecutionConfig,

    /// Workflow steps
    pub steps: Vec<TaskfileStep>,

    /// Output contract
    #[serde(default)]
    pub output: Output,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskfileMetadata {
    pub name: String,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default = "default_version")]
    pub version: String,

    #[serde(default)]
    pub author: Option<String>,

    #[serde(default)]
    pub tags: Vec<String>,
}

fn default_version() -> String {
    "1.0.0".to_string()
}

// === TRIGGERS ===

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Triggers {
    #[serde(default)]
    pub manual: ManualTrigger,

    #[serde(default)]
    pub cron: Option<CronTrigger>,

    #[serde(default)]
    pub http: Option<HttpTrigger>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualTrigger {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for ManualTrigger {
    fn default() -> Self {
        ManualTrigger { enabled: true }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronTrigger {
    #[serde(default)]
    pub enabled: bool,

    /// Cron expression (e.g., "0 9 * * 1-5")
    pub expression: String,

    /// Timezone (e.g., "America/New_York")
    #[serde(default = "default_timezone")]
    pub timezone: String,
}

fn default_timezone() -> String {
    "UTC".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpTrigger {
    #[serde(default)]
    pub enabled: bool,

    /// Webhook path (e.g., "/webhook/login")
    pub path: String,

    /// HTTP method
    #[serde(default = "default_method")]
    pub method: String,

    /// Authentication config
    #[serde(default)]
    pub auth: Option<HttpAuth>,
}

fn default_method() -> String {
    "POST".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpAuth {
    /// Auth type: none | api_key | bearer | hmac
    #[serde(rename = "type")]
    pub auth_type: String,

    /// Header name for the auth token
    #[serde(default)]
    pub header: Option<String>,

    /// Environment variable containing the secret
    #[serde(default)]
    pub secret_env: Option<String>,
}

// === DEPENDENCIES ===

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Dependencies {
    #[serde(default)]
    pub browser: BrowserDependency,

    #[serde(default)]
    pub env: Vec<EnvDependency>,

    #[serde(default)]
    pub accounts: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowserDependency {
    /// Browser type: chromium | firefox | webkit
    #[serde(rename = "type", default = "default_browser_type")]
    pub browser_type: String,

    #[serde(default)]
    pub headless: bool,
}

impl Default for BrowserDependency {
    fn default() -> Self {
        BrowserDependency {
            browser_type: "chromium".to_string(),
            headless: false,
        }
    }
}

fn default_browser_type() -> String {
    "chromium".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvDependency {
    pub name: String,

    #[serde(default)]
    pub required: bool,

    #[serde(default)]
    pub sensitive: bool,

    #[serde(default)]
    pub default: Option<String>,

    #[serde(default)]
    pub description: Option<String>,
}

// === LIMITS ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Limits {
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u32,

    #[serde(default = "default_max_steps")]
    pub max_steps: u32,

    #[serde(default)]
    pub network: Option<NetworkLimits>,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            timeout_seconds: 300,
            max_steps: 100,
            network: None,
        }
    }
}

fn default_timeout() -> u32 {
    300
}

fn default_max_steps() -> u32 {
    100
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkLimits {
    #[serde(default)]
    pub allowed_domains: Vec<String>,
}

// === VARIABLES ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,

    /// Variable type: string | number | boolean
    #[serde(rename = "type", default = "default_var_type")]
    pub var_type: String,

    #[serde(default)]
    pub required: bool,

    #[serde(default)]
    pub default: Option<serde_json::Value>,

    #[serde(default)]
    pub description: Option<String>,
}

fn default_var_type() -> String {
    "string".to_string()
}

// === EXECUTION ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionConfig {
    /// Execution mode: direct | ai_assisted
    #[serde(default = "default_mode")]
    pub mode: String,

    #[serde(default)]
    pub llm: Option<LLMExecutionConfig>,

    #[serde(default)]
    pub retry: RetryConfig,
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        ExecutionConfig {
            mode: "ai_assisted".to_string(),
            llm: None,
            retry: RetryConfig::default(),
        }
    }
}

fn default_mode() -> String {
    "ai_assisted".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMExecutionConfig {
    pub provider: String,
    pub model: String,

    /// Environment variable containing the API key
    #[serde(default)]
    pub api_key_env: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetryConfig {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,

    #[serde(default = "default_delay_ms")]
    pub delay_ms: u32,
}

impl Default for RetryConfig {
    fn default() -> Self {
        RetryConfig {
            max_attempts: 3,
            delay_ms: 1000,
        }
    }
}

fn default_max_attempts() -> u32 {
    3
}

fn default_delay_ms() -> u32 {
    1000
}

// === STEPS ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskfileStep {
    pub id: String,

    pub action: TaskfileAction,

    #[serde(default)]
    pub description: Option<String>,

    /// Condition to execute this step
    #[serde(default)]
    pub condition: Option<StepCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskfileAction {
    Navigate {
        url: String,
    },
    Click {
        selector: Selector,
    },
    Type {
        selector: Selector,
        text: String,
        #[serde(default)]
        clear_first: bool,
    },
    Wait {
        condition: WaitCondition,
    },
    Extract {
        selector: Selector,
        #[serde(default = "default_attribute")]
        attribute: String,
        variable: String,
    },
    Screenshot {
        #[serde(default)]
        full_page: bool,
        #[serde(default)]
        variable: Option<String>,
    },
    Scroll {
        #[serde(default)]
        direction: String,
        #[serde(default)]
        amount: Option<i32>,
    },
    Select {
        selector: Selector,
        value: String,
    },
    Hover {
        selector: Selector,
    },
    Custom {
        prompt: String,
    },
}

fn default_attribute() -> String {
    "textContent".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Selector {
    #[serde(default)]
    pub css: Option<String>,

    #[serde(default)]
    pub xpath: Option<String>,

    #[serde(default)]
    pub text: Option<String>,

    #[serde(default)]
    pub aria_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WaitCondition {
    UrlMatch {
        value: String,
        #[serde(default = "default_wait_timeout")]
        timeout_ms: u32,
    },
    ElementVisible {
        selector: Selector,
        #[serde(default = "default_wait_timeout")]
        timeout_ms: u32,
    },
    ElementHidden {
        selector: Selector,
        #[serde(default = "default_wait_timeout")]
        timeout_ms: u32,
    },
    Delay {
        ms: u32,
    },
}

fn default_wait_timeout() -> u32 {
    10000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepCondition {
    /// Variable name to check
    pub variable: String,

    /// Operator: eq | ne | contains | exists
    pub operator: String,

    /// Value to compare against
    #[serde(default)]
    pub value: Option<serde_json::Value>,
}

// === OUTPUT ===

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Output {
    #[serde(default)]
    pub variables: Vec<String>,

    #[serde(default)]
    pub screenshots: ScreenshotOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotOutput {
    #[serde(default)]
    pub include: bool,

    #[serde(default = "default_format")]
    pub format: String,
}

impl Default for ScreenshotOutput {
    fn default() -> Self {
        ScreenshotOutput {
            include: true,
            format: "png".to_string(),
        }
    }
}

fn default_format() -> String {
    "png".to_string()
}

// === VALIDATION RESULT ===

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    pub path: String,
    pub message: String,
}
