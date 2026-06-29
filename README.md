# 🚀 TMX 10x - The Ultimate Agentic Workspace

`tmx` is a blazing-fast, Rust-based CLI wrapper around `tmux` designed specifically for **AI Developers** and **Multi-Agent Orchestration**.

Managing multiple autonomous AI agents (like Claude Code, OpenDevin, Aider) in the terminal often leads to "terminal sprawl", lost scrollback logs, and a confusing mess of windows. `tmx` solves all of this by automatically configuring `tmux` with powerful features tailored for AI development—without modifying your global `~/.tmux.conf`.

## ✨ Features

- **🏗️ Dynamic Layouts:** Instantly spin up Dev (3-pane), Swarm (4-pane), or Observer (2-pane with `htop`) layouts.
- **🌐 Global Dashboard:** Run `tmx ps` to see a global dashboard of all running agents and their last 3 lines of output across all sessions.
- **📋 Instant Yank:** Press `Prefix + y` to instantly copy the last 200 lines of agent output directly to your system clipboard (bypasses tmux copy-mode).
- **🖨️ Auto-Logger:** Press `Prefix + P` to toggle piping the active pane's output to a log file on your hard drive. Never lose an agent's trace again!
- **🔒 Safe Lock Mode:** Prevent accidental keystrokes from interrupting an AI agent by hitting `Prefix + L` to lock keyboard input for that pane.
- **💾 1-Click Crash Reporter:** Press `Prefix + S` to save the active pane's scrollback history to a `tmx_crash.log` file in the current directory.
- **⚡ Popup Switcher:** Hit `Prefix + w` for an interactive, centralized menu to jump between active workspaces.
- **🧠 Muscle-Memory Builder:** Full mouse support enabled! Every time you use the mouse to click or resize, `tmx` displays a toast notification teaching you the keyboard shortcut.

## 🛠️ Installation

You can install `tmx` via our installation script:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/krunalparmarem/tmx/main/install.sh | bash
```

*Note: Requires `tmux` and `cargo`/`rust` to be installed on your system.*

## 🚀 Quick Start

Run `tmx` from your terminal to launch the interactive menu.

```bash
tmx
```

### Initial Setup
The very first time you run `tmx`, it will interactively prompt you to configure:
1. **Your Prefix Key** (e.g., `C-a`, `Option-Space`). You can simply *press* the key!
2. **Your Editor Command** (e.g., `cursor .`, `code .`, `vim`)
3. **Your Environment Command** (e.g., `source venv/bin/activate`, `nvm use 18`)
4. **Your Workspace Root** (e.g., `~/Projects` — where agent project directories are created)

*Need to change your settings later? Just run `tmx config`!*

### CLI Commands

```bash
tmx agent <project_name>   # Instantly launch a new agent workspace
tmx agent <name> --no-attach  # Create workspace detached (for scripts/CI)
tmx attach                 # Attach to a session (auto-attaches if only one exists)
tmx attach -s <name>       # Attach to a specific session (non-interactive friendly)
tmx switch                 # Open the interactive workspace switcher
tmx switch -s <name>       # Switch to a specific session
tmx ps                     # View the global agent dashboard
tmx monitor                # Launch the live-updating full-screen dashboard
tmx cheat                  # View your colorized keyboard cheatsheet
tmx config                 # Change prefix, editor, environment, or workspace settings
tmx kill                   # Kill an existing workspace
tmx kill -s <name>         # Kill a specific session (non-interactive friendly)
```

## ⌨️ Power Shortcuts

Assuming your prefix is `C-a`:

| Shortcut | Action |
| --- | --- |
| `Option + Arrows` | Instantly move between panes (no prefix required) |
| `C-a` + `y` | Yank last 200 lines to clipboard |
| `C-a` + `P` | Toggle Auto-Logger to hard drive |
| `C-a` + `L` | Lock Pane (Disable Keyboard) |
| `C-a` + `U` | Unlock Pane |
| `C-a` + `S` | Save Crash Report |
| `C-a` + `g` | Open Floating Scratchpad (Popup) |
| `C-a` + `s` | Synchronize typing across all panes |
| `C-a` + `w` | Open Quick Switcher Popup |
| `C-a` + `/` | Search through scrollback backwards |
| `C-a` + `d` | Detach from session (leaves it running) |
| `C-a` + `z` | Zoom pane to fullscreen |

### Clipboard on Linux

The yank shortcut (`Prefix + y`) requires a clipboard tool:
- **macOS:** `pbcopy` (built-in)
- **Wayland:** `wl-copy`
- **X11:** `xclip`

## 🤝 Contributing
Pull requests are welcome! If you have ideas for new Agentic features, please open an issue!

## 📄 License
MIT License
