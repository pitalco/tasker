# Tasker

**Open-source agent builder to automate anything.**

Record a task as a template. Feed it into Tasker with additional instructions and watch magic happen.

## How It Works

1. **Record** - Click "Start Recording" and perform the task you want to automate in Chrome
2. **Save** - Stop recording to save your workflow as a reusable template
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
│   ├── routes/             # Pages (record, workflows, settings)
│   └── lib/                # Components and services
├── src-tauri/              # Tauri backend (Rust)
└── tasker-sidecar/         # Browser automation engine (Rust)
    ├── src/
    │   ├── api/            # HTTP/WebSocket server
    │   ├── browser/        # Chrome automation (CDP)
    │   ├── recording/      # Workflow recording
    │   ├── replay/         # Workflow replay
    │   └── runs/           # AI execution
    └── tests/              # Integration tests
```

## Tech Stack

| Layer | Technology |
|-------|------------|
| Frontend | SvelteKit 2, Tailwind CSS |
| Desktop | Tauri 2 |
| Automation | Rust, chromiumoxide (CDP) |
| AI | Claude, OpenAI, Gemini via genai |

## Configuration

Add your LLM API key in Settings. Supported providers:

- **Anthropic**
- **OpenAI**
- **Google**

## Contributing

Contributions welcome! Please read our contributing guidelines before submitting PRs.

## License

Tasker is open source for personal and non-commercial use. Commercial use requires a separate license. See [LICENSE](LICENSE) for details.
