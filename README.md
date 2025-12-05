# Tasker

**Open-source agent builder to automate anything in the browser.**

Create workflows by recording browser actions, then let AI execute them intelligently.

## How It Works

1. **Create** - Click "Create Workflow" and start recording your browser actions
2. **Record** - Perform the task you want to automate in Chrome
3. **Run** - Execute the workflow with AI that adapts to dynamic content

## Taskfiles

Taskfiles are to browser automation what Dockerfiles are to containers - **portable, shareable, and extensible**.

```json
{
  "name": "Submit expense report",
  "start_url": "https://expenses.company.com",
  "actions": [
    { "tool": "click_element", "hints": { "text": "New Report" } },
    { "tool": "input_text", "params": { "text": "{{description}}" } },
    { "tool": "click_element", "hints": { "text": "Submit" } }
  ]
}
```

**Why Taskfiles?**

- **Version Control** - Track changes to your automations in Git
- **Share & Reuse** - Export workflows and share with your team
- **Parameterized** - Use variables like `{{description}}` for dynamic input
- **Human Readable** - JSON format that's easy to understand and edit
- **AI-Enhanced** - Hints guide the AI to find elements even when pages change

## Features

- **Visual Recording** - No code required. Just click, type, and interact naturally
- **Taskfiles** - Portable workflow format you can version, share, and customize
- **AI-Powered Execution** - LLMs understand context and adapt to page changes
- **Multi-Provider** - Works with Claude, OpenAI, or Gemini
- **Desktop App** - Native performance with Tauri (Windows, macOS, Linux)
- **File Management** - View, download, and manage files created during runs
- **Stop When Conditions** - Define completion criteria (e.g., "collected at least 10 results")
- **Configurable Max Steps** - Set global defaults or per-workflow limits

### Automation Targets

- [x] **Browser** - Automate anything in the browser with Chrome
- [ ] **OS** - Full desktop automation (coming soon)

## Tasker Deploy (Coming Soon)

Deploy your Taskfiles to the cloud with one click.

- **Scheduled Runs** - Set up recurring runs of Tasks (hourly, daily, weekly)
- **HTTP Triggers** - Trigger Taskfiles via API endpoints
- **Webhooks** - Connect to Zapier, n8n, or your own services
- **Dashboard** - Monitor runs, view logs, and track usage

## Getting Started

### Prerequisites

- [Bun](https://bun.sh/) (or Node.js)
- [Rust](https://rustup.rs/)
- Chrome/Chromium browser
- API key for Claude, OpenAI, or Gemini

### Installation

```bash
# Clone the repo
git clone https://github.com/pitalco/tasker.git
cd tasker

# Install dependencies
bun install

# Build the sidecar
cd tasker-sidecar
cargo build --release
cd ..

# Run in development mode
bun run tauri dev
```

### Building for Production

```bash
bun run tauri build
```

## Project Structure

```
tasker/
├── src/                    # SvelteKit frontend
│   ├── routes/             # Pages (workflows, runs, files, settings)
│   └── lib/                # Components and services
├── src-tauri/              # Tauri backend (Rust)
│   ├── src/
│   │   ├── commands/       # Tauri command handlers
│   │   ├── db/             # SQLite database layer
│   │   └── taskfile/       # Taskfile import/export
└── tasker-sidecar/         # Browser automation engine (Rust)
    ├── src/
    │   ├── api/            # HTTP/WebSocket server
    │   ├── browser/        # Chrome automation (CDP)
    │   ├── recording/      # Workflow recording
    │   └── runs/           # AI execution engine
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Frontend | SvelteKit 2, Tailwind CSS |
| Desktop | Tauri 2 |
| Automation | Rust, chromiumoxide (CDP) |
| AI | Claude, OpenAI, Gemini via genai |
| Database | SQLite |

## Configuration

Add your LLM API key in Settings. Supported providers:

- **Anthropic** (Claude)
- **OpenAI** (GPT-4, etc.)
- **Google** (Gemini)

### Execution Settings

- **Default Max Steps** - Maximum steps before a run stops (default: 50)
- **Per-Workflow Override** - Set custom max steps for specific workflows
- **Stop When** - Define completion conditions for workflows

## Contributing

Contributions welcome! Please read our contributing guidelines before submitting PRs.

## License

MIT - See [LICENSE](LICENSE) for details.
