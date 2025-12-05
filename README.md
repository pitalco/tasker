# Tasker

**AI-powered browser automation that adapts to any website.**

Record workflows by example, then let an AI agent execute them intelligently. Unlike brittle macro recorders, Tasker's AI understands what you're trying to accomplish and adapts to page changes, popups, and dynamic content.

## How It Works

1. **Create Workflow** - Click "Create Workflow" and choose to record or describe your task
2. **Record** - Perform the task in Chrome while Tasker captures your actions
3. **Run** - AI agent executes the task, using your recording as guidance

### AI-First Execution

This is not a macro player. When you run a workflow:

- Recorded steps serve as **hints**, not a strict script
- The AI agent sees the actual page and decides what to do
- It handles popups, layout changes, CAPTCHAs, and dynamic content
- If something doesn't match the recording, it adapts

## Features

- **Visual Recording** - Record browser actions without writing code
- **Multi-Provider AI** - Choose Claude, GPT-4, or Gemini for execution
- **Variables** - Parameterize workflows with dynamic inputs
- **Stop Conditions** - Define when the task is complete (e.g., "collected 10 results")
- **Max Steps** - Prevent runaway executions with configurable limits
- **File Management** - View and download files created during runs
- **Taskfile Export** - Share workflows as portable YAML files
- **Headless Mode** - Run in background without visible browser
- **Run History** - Review step-by-step execution logs with screenshots

## Supported LLM Providers

| Provider | Models |
|----------|--------|
| Anthropic | Claude Sonnet 4, Claude Haiku 4.5 |
| OpenAI | GPT-4o, GPT-4o-mini |
| Google | Gemini 2.5 Flash, Gemini 2.5 Pro, Gemini 3 Pro |

Configure API keys in Settings. Only models with configured keys are available.

## Taskfile Format

Export workflows as portable YAML files:

```yaml
taskfile: "1.0"
metadata:
  name: "Search for products"
  description: "Search an e-commerce site and extract results"
  version: "1.0.0"

variables:
  - name: search_term
    type: string
    required: true
  - name: max_results
    type: number
    default: 10

execution:
  mode: ai_assisted
  llm:
    provider: anthropic
    model: claude-sonnet-4-20250514

limits:
  timeout_seconds: 300
  max_steps: 50

steps:
  - id: navigate
    action:
      type: navigate
      url: "https://example-store.com"

  - id: search
    action:
      type: type
      selector:
        css: "input[name='search']"
      text: "{{search_term}}"

  - id: submit
    action:
      type: click
      selector:
        text: "Search"

  - id: extract
    action:
      type: extract
      selector:
        css: ".product-card"
      attribute: textContent
      variable: results
```

### Supported Actions

| Action | Description |
|--------|-------------|
| `navigate` | Go to URL |
| `click` | Click element |
| `type` | Enter text (with optional `clear_first`) |
| `wait` | Wait for condition (element visible/hidden, URL match, delay) |
| `extract` | Extract text/attributes to variable |
| `scroll` | Scroll up/down |
| `select` | Choose dropdown option |
| `hover` | Hover over element |
| `screenshot` | Capture page |
| `custom` | Natural language instruction for AI |

### Selectors

Elements can be targeted by:
- `css` - CSS selector
- `xpath` - XPath expression
- `text` - Text content match
- `aria_label` - ARIA label

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri Desktop App                     │
│  ┌─────────────────┐    ┌────────────────────────────┐  │
│  │  SvelteKit UI   │◄──►│     Tauri Commands         │  │
│  │  (Frontend)     │    │  (Settings, Workflows DB)  │  │
│  └─────────────────┘    └────────────────────────────┘  │
└────────────────────────────┬────────────────────────────┘
                             │ HTTP/WebSocket
                             ▼
┌─────────────────────────────────────────────────────────┐
│              Sidecar Service (Port 8765)                 │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐  │
│  │  Recording  │  │  AI Agent   │  │  Run Repository │  │
│  │  Engine     │  │  Executor   │  │  (SQLite)       │  │
│  └──────┬──────┘  └──────┬──────┘  └─────────────────┘  │
│         │                │                               │
│         ▼                ▼                               │
│  ┌─────────────────────────────────────────────────┐    │
│  │         Browser Manager (chromiumoxide/CDP)      │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

- **Tauri App** - Desktop shell, manages sidecar lifecycle, stores settings
- **Sidecar** - Rust HTTP server handling browser automation and AI execution
- **WebSocket** - Real-time step updates during recording and execution

## Project Structure

```
tasker/
├── src/                        # SvelteKit frontend
│   ├── routes/                 # Pages
│   │   ├── (home)              # Workflow list
│   │   ├── workflows/[id]      # Edit workflow
│   │   ├── replay/[id]         # Run workflow
│   │   ├── runs/               # Run history
│   │   ├── files/              # File browser
│   │   └── settings/           # Configuration
│   └── lib/
│       ├── components/         # UI components
│       ├── services/           # API clients
│       └── types/              # TypeScript types
├── src-tauri/                  # Tauri backend
│   └── src/
│       ├── commands/           # IPC handlers
│       ├── db/                 # SQLite (workflows, settings)
│       └── taskfile/           # YAML import/export
└── tasker-sidecar/             # Browser automation engine
    └── src/
        ├── api/                # HTTP endpoints
        ├── browser/            # Chrome control (CDP)
        ├── recording/          # Action capture
        ├── runs/               # AI execution
        ├── tools/              # Browser tools for AI
        └── llm/                # LLM client & prompts
```

## Tech Stack

| Component | Technology |
|-----------|------------|
| Frontend | SvelteKit 2, Tailwind CSS |
| Desktop | Tauri 2 (Rust) |
| Sidecar | Rust, Axum, tokio |
| Browser | chromiumoxide (Chrome DevTools Protocol) |
| Database | SQLite |
| LLM | genai crate (multi-provider) |

## Getting Started

### Prerequisites

- [Bun](https://bun.sh/) or Node.js 18+
- [Rust](https://rustup.rs/) 1.70+
- Chrome or Chromium browser
- API key for at least one LLM provider

### Installation

```bash
# Clone the repository
git clone https://github.com/pitalco/tasker.git
cd tasker

# Install frontend dependencies
bun install

# Build the sidecar
cd tasker-sidecar
cargo build --release
cd ..

# Run in development mode
bun run tauri dev
```

### Production Build

```bash
bun run tauri build
```

The built app will be in `src-tauri/target/release/`.

## Configuration

### API Keys

Add your LLM API keys in **Settings**:
- Anthropic API key for Claude models
- OpenAI API key for GPT models
- Google API key for Gemini models

### Execution Defaults

- **Default Max Steps** - Global limit (default: 50), prevents infinite loops
- **Per-Workflow Override** - Set custom limits for specific workflows
- **Stop When** - Define completion conditions per workflow

## Workflow Settings

Each workflow can configure:

| Setting | Description |
|---------|-------------|
| Name | Display name |
| Task Description | What the workflow accomplishes (used by AI) |
| Variables | Input parameters with types and defaults |
| Stop When | Condition for task completion |
| Max Steps | Override global step limit |

## Run Options

When executing a workflow:

| Option | Description |
|--------|-------------|
| LLM Provider | Which AI to use |
| Model | Specific model from provider |
| Iterations | Run multiple times (1-100) |
| Headless | Run without visible browser |
| Custom Instructions | Additional guidance for AI |

## Contributing

Contributions welcome! Please read our contributing guidelines before submitting PRs.

## License

MIT - See [LICENSE](LICENSE) for details.
