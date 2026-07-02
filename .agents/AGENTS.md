# TMX 10x Workspace Agent Rules

These rules apply specifically to the `tmx` project, a Rust-based CLI wrapper around `tmux` for AI development.

## 🏗️ Code Quality & Conventions
- **Rust Toolchain:** Always ensure code compiles with `cargo check` and contains no warnings. Use `cargo fmt` for formatting and address all `cargo clippy` suggestions.
- **Error Handling:** Avoid using `.unwrap()` or `.expect()` where failure is likely (like missing environment variables, file reads, or command execution). Provide clear, user-friendly error messages since this is a CLI tool meant for humans.
- **Dependencies:** Limit adding new heavy dependencies unless necessary. We prefer lightweight execution to maintain the "blazing-fast" goal.

## 🧠 Design Philosophy
- **Interactive First:** Prefer using interactive prompts (e.g., via `dialoguer` or `inquire`) for missing configurations rather than outright failing.
- **Non-Invasive:** `tmx` should never modify a user's global `~/.tmux.conf` file directly. Always generate configuration dynamically in `~/.config/tmx/tmx.conf` and pass it to `tmux` using the `-f` flag.
- **Focus on the AI Developer:** New features should cater to the pain points of working with multiple LLM-driven agents (e.g., preserving logs, isolating execution, or monitoring multiple outputs).

## 📝 Documentation — Required After Every Change

**Every code change must include documentation updates in the same PR/commit.** Do not merge behavior changes without updating docs.

### Documentation checklist

When you change code, update **every applicable item** before finishing:

| Change type | Update these |
| --- | --- |
| New/changed CLI subcommand or flag | `README.md` CLI section, `tmx --help` text (clap doc comments in `main.rs`) |
| New/changed `Prefix` shortcut | `src/tmx.conf`, `print_cheatsheet()` in `main.rs`, `README.md` power shortcuts table |
| New/changed workspace layout | `README.md` layouts section, `print_layout_preview()` / art in `src/ui.rs` |
| New/changed config field | `README.md` configuration section, setup/config prompts in `main.rs`, `AppConfig` serde defaults |
| New/changed install dependency | `README.md` installation notes, `install.sh`, `.github/workflows/release.yml`, `packaging/tmx.rb` |
| New user-visible UI/ASCII art | `src/ui.rs` (if applicable), `README.md` if user-facing |
| Scripting / non-interactive behavior | `README.md` scripting section |
| New/changed swarm or git-worktree behavior | `src/git.rs`, `src/state.rs`, `run_swarm`/`run_review` in `main.rs`, `README.md` swarm section |
| New/changed agent preset | `KNOWN_AGENTS` in `main.rs`, `README.md` agents table |

## 🐝 Swarm architecture notes (for agents)

- **Isolation invariant:** every swarm agent MUST get its own `git worktree` + branch. Never make swarm panes share a working directory — that is the whole point of the feature.
- **Modules:** `src/git.rs` wraps the `git` CLI (shell out, no deps); `src/state.rs` persists swarms to `~/.config/tmx/state.json` so `tmx review` works across process restarts.
- **Worktrees** live at `<repo>/.tmx-trees/<session>/<codename>` on branches `tmx/<session>/<codename>`. `.tmx-trees/` is auto-added to the repo `.gitignore`.
- **Rollback:** if swarm setup fails partway, remove created worktrees + branches AND kill the tmux session (see `cleanup_worktrees` + `SessionGuard`).
- **Notifications:** fire via shelling out only (`osascript`/`terminal-notifier`/`notify-send`) — do not add notification crates.

### Files that must stay in sync

1. **`README.md`** — primary user-facing documentation (features, CLI, layouts, config, shortcuts)
2. **`src/main.rs`** — clap `///` doc comments on subcommands and flags
3. **`print_cheatsheet()`** in `main.rs` — must match `README.md` shortcuts and `src/tmx.conf` bindings
4. **`src/tmx.conf`** — source template for generated `~/.config/tmx/tmx.conf`
5. **`install.sh`** — install requirements and repo URL
6. **`.github/workflows/release.yml`** + **`packaging/tmx.rb`** — release targets and Homebrew formula (keep in sync with supported platforms)

### Rule

> If a user would notice the change, document it. If an agent would need to know the change, document it in `AGENTS.md` or inline code comments for non-obvious logic only.
