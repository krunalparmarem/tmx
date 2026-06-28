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

## 📝 Documentation
- When adding new `Prefix` shortcuts, ensure they are documented in both the `tmx cheat` command output and the `README.md` power shortcuts table.
- Keep the `install.sh` and `README.md` in sync if the setup or dependencies change.
