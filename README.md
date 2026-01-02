# Tasker

**AI-powered browser automation that adapts to any website.**

Record workflows by example, then let an AI agent execute them intelligently. Unlike brittle macro recorders, Tasker's AI understands what you're trying to accomplish and adapts to page changes, popups, and dynamic content.

## How It Works

1. **Create Workflow** - Record a browser task OR describe it in plain English
2. **Recording Becomes Hints** - Your actions are captured as guidance, not as a macro to replay
3. **AI Executes Intelligently** - The AI sees your hints + the current page and decides how to proceed

### AI-First, Not Macro Playback

Tasker is fundamentally different from traditional automation:

| Traditional RPA | Tasker |
|----------------|--------|
| Replays exact coordinates/selectors | AI understands the *intent* behind actions |
| Breaks when UI changes | Adapts to layout changes, popups, dynamic content |
| Blind execution | Sees the page via screenshots + DOM analysis |
| Fixed script | Can deviate from recording when needed |

Your recorded workflow becomes **semantic context** for the AI. It knows "the user clicked a Submit button" rather than "click at (234, 567)".

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
- **Agent Memory** - AI can save notes and recall information across steps

## Supported LLM Providers

| Provider | Models |
|----------|--------|
| Anthropic | Claude Opus 4.5, Claude Sonnet 4.5, Claude Haiku 4.5 |
| OpenAI | GPT-4o, GPT-4o Mini |
| Google | Gemini 2.5 Flash, Gemini 2.5 Pro, Gemini 3 Flash, Gemini 3 Pro |

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
    model: claude-sonnet-4-5

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
│   │   ├── +page.svelte        # Home - workflow list
│   │   ├── record/             # Recording interface
│   │   ├── workflows/          # Create & edit workflows
│   │   ├── replay/[id]/        # Run workflow
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

## License

MIT - See [LICENSE](LICENSE) for details.
