mod git;
mod state;
mod theme;
mod ui;

use clap::{Parser, Subcommand};
use dialoguer::{Confirm, Input, Select};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use theme::tmx_theme;
use ui::{
    AgentState, BOLD, DIM, MAUVE, OBSERVER_MONITOR, OBSERVER_SHELL, RESET, SKY, SUBTEXT,
    TAB_COLORS, TEXT, YELLOW, ansi_bg_fill_shell, attach_existing, cheat_row, cheat_section,
    clear_screen, config_banner, dev_pane_splash, die, doctor_check, doctor_hint, heading,
    highlight, info, menu_banner, monitor_footer, no_sessions, pane_card, pane_card_end,
    pane_log_line_styled, pane_splash_command, pause_menu, print_cheatsheet_header,
    print_dashboard_header, print_dashboard_stats, print_doctor_banner, print_goodbye,
    print_layout_preview, print_review_banner, print_setup_banner, print_swarm_banner,
    print_switch_banner, review_agent_card, section, section_end, session_created, session_killed,
    spawn_ritual, success, swarm_pane_bg, swarm_pane_splash_script, warn,
    window_color_script_body,
};

const TMX_CONF: &str = include_str!("tmx.conf");

#[derive(Serialize, Deserialize, Clone)]
struct AppConfig {
    prefix: String,
    editor_cmd: String,
    env_cmd: String,
    #[serde(default)]
    workspace_root: String,
    /// Map of agent name -> launch command template (use `{task}` for the prompt).
    #[serde(default)]
    agents: BTreeMap<String, String>,
    /// Preferred agent name for swarms (key into `agents`).
    #[serde(default)]
    default_agent: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            prefix: "C-a".to_string(),
            editor_cmd: String::new(),
            env_cmd: String::new(),
            workspace_root: String::new(),
            agents: BTreeMap::new(),
            default_agent: String::new(),
        }
    }
}

/// Known agent CLIs: (binary, display name, command template).
const KNOWN_AGENTS: &[(&str, &str, &str)] = &[
    ("claude", "Claude Code", "claude {task}"),
    ("codex", "Codex CLI", "codex {task}"),
    ("cursor-agent", "Cursor CLI", "cursor-agent {task}"),
    ("aider", "Aider", "aider --message {task}"),
    ("gemini", "Gemini CLI", "gemini {task}"),
    ("opencode", "OpenCode", "opencode {task}"),
];

const CODENAMES: [&str; 4] = ["alpha", "bravo", "charlie", "delta"];

#[derive(Parser)]
#[command(name = "tmx", about = "10x Agentic Tmux Workspace", version)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Agent Workspace
    Agent {
        /// Name of the session
        name: Option<String>,
        /// Create the session detached without attaching
        #[arg(long, default_value_t = false)]
        no_attach: bool,
    },
    /// Spawn parallel AI agents, each isolated in its own git worktree + branch
    Swarm {
        /// What the agents should build (plain words, no quotes needed)
        #[arg(trailing_var_arg = true)]
        task: Vec<String>,
        /// Number of parallel agents (2-4, default 3)
        #[arg(short = 'n', long)]
        agents: Option<usize>,
        /// Create the swarm detached without attaching
        #[arg(long, default_value_t = false)]
        no_attach: bool,
    },
    /// Review a swarm's agents and keep the best (merges its branch)
    Review {
        /// Swarm session name (skips interactive picker)
        #[arg(short, long)]
        session: Option<String>,
    },
    /// Attach to an existing session
    Attach {
        /// Session name (skips interactive picker)
        #[arg(short, long)]
        session: Option<String>,
    },
    /// Quick switch between active sessions
    Switch {
        /// Session name (skips interactive picker)
        #[arg(short, long)]
        session: Option<String>,
    },
    /// Global Dashboard: List all agents and their latest logs
    Ps,
    /// Global Dashboard: Interactive live-updating monitor
    Monitor,
    /// Kill an existing session
    Kill {
        /// Session name (skips interactive picker)
        #[arg(short, long)]
        session: Option<String>,
    },
    /// Change prefix, editor, environment, or workspace settings
    Config,
    /// Diagnose your setup (tmux, git, terminal, clipboard, agents)
    Doctor,
    /// View the Tmux cheatsheet
    Cheat,
}

struct MonitorGuard;

impl Drop for MonitorGuard {
    fn drop(&mut self) {
        let _ = write!(io::stdout(), "\x1B[?1049l\x1B[?25h");
    }
}

/// Rolls back a newly created tmux session if setup or attach fails.
struct SessionGuard {
    name: String,
    keep: bool,
}

impl SessionGuard {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            keep: false,
        }
    }

    fn keep(mut self) {
        self.keep = true;
        std::mem::forget(self);
    }
}

impl Drop for SessionGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = Command::new("tmux")
                .args(["kill-session", "-t", &self.name])
                .status();
        }
    }
}

fn is_interactive() -> bool {
    io::stdin().is_terminal()
}

fn can_attach() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

fn home_dir() -> Result<PathBuf, String> {
    std::env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| "HOME environment variable is not set".to_string())
}

fn config_dir() -> Result<PathBuf, String> {
    Ok(home_dir()?.join(".config").join("tmx"))
}

fn expand_tilde(path: &str, home: &Path) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        home.join(rest)
    } else if path == "~" {
        home.to_path_buf()
    } else {
        PathBuf::from(path)
    }
}

fn resolve_workspace_root(cfg: &AppConfig) -> Result<PathBuf, String> {
    let home = home_dir()?;
    if cfg.workspace_root.trim().is_empty() {
        Ok(home.join("Projects"))
    } else {
        Ok(expand_tilde(cfg.workspace_root.trim(), &home))
    }
}

fn tmx_bin() -> String {
    std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "tmx".to_string())
}

fn shell_cmd() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string())
}

fn clipboard_cmd() -> String {
    if cfg!(target_os = "macos") {
        return "pbcopy".to_string();
    }
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        return "wl-copy".to_string();
    }
    if cfg!(target_os = "linux") {
        return "xclip -selection clipboard".to_string();
    }
    "cat".to_string()
}

fn clipboard_available() -> bool {
    clipboard_cmd() != "cat"
}

fn state_path() -> Result<PathBuf, String> {
    Ok(config_dir()?.join("state.json"))
}

/// True if `bin` is an executable on the user's PATH.
fn is_on_path(bin: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|dir| dir.join(bin).is_file()))
        .unwrap_or(false)
}

/// POSIX single-quote a string so it is safe to send to a shell via send-keys.
fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Build the concrete command to launch an agent, substituting the task prompt.
fn agent_invocation(template: &str, task: &str) -> String {
    if template.contains("{task}") {
        template.replace("{task}", &shell_quote(task))
    } else if task.trim().is_empty() {
        template.to_string()
    } else {
        format!("{template} {}", shell_quote(task))
    }
}

/// Detect known agent CLIs available on PATH.
fn detect_agents() -> BTreeMap<String, String> {
    let mut m = BTreeMap::new();
    for (bin, _display, tmpl) in KNOWN_AGENTS {
        if is_on_path(bin) {
            m.insert((*bin).to_string(), (*tmpl).to_string());
        }
    }
    m
}

fn choose_default_agent(agents: &BTreeMap<String, String>) -> Result<String, String> {
    if agents.is_empty() {
        return Ok(String::new());
    }
    if !is_interactive() {
        return Ok(String::new());
    }
    let keys: Vec<String> = agents.keys().cloned().collect();
    // Offer an explicit "no default" choice so the setting stays optional —
    // the swarm will prompt for an agent at spawn time when left empty.
    const NONE_LABEL: &str = "(none — ask each time)";
    let mut items: Vec<String> = Vec::with_capacity(keys.len() + 1);
    items.push(NONE_LABEL.to_string());
    items.extend(keys.iter().cloned());
    let idx = Select::with_theme(&tmx_theme())
        .with_prompt("Default agent for swarms")
        .default(0)
        .items(&items)
        .interact()
        .map_err(|e| format!("agent selection failed: {e}"))?;
    if idx == 0 {
        return Ok(String::new());
    }
    Ok(keys[idx - 1].clone())
}

/// Resolve which agent command template a swarm should run.
fn resolve_agent_cmd(cfg: &AppConfig) -> Result<String, String> {
    let agents = if cfg.agents.is_empty() {
        detect_agents()
    } else {
        cfg.agents.clone()
    };

    if agents.is_empty() {
        if is_interactive() {
            let tmpl: String = Input::with_theme(&tmx_theme())
                .with_prompt("Agent command to run in each pane (use {task} for the prompt)")
                .default("claude {task}".to_string())
                .interact()
                .map_err(|e| format!("agent prompt failed: {e}"))?;
            return Ok(tmpl);
        }
        return Ok(cfg.env_cmd.clone());
    }

    if let Some(tmpl) = agents.get(&cfg.default_agent) {
        return Ok(tmpl.clone());
    }
    let keys: Vec<String> = agents.keys().cloned().collect();
    if keys.len() == 1 || !is_interactive() {
        return Ok(agents[&keys[0]].clone());
    }
    let idx = Select::with_theme(&tmx_theme())
        .with_prompt("Which agent should the swarm run?")
        .default(0)
        .items(&keys)
        .interact()
        .map_err(|e| format!("agent selection failed: {e}"))?;
    Ok(agents[&keys[idx]].clone())
}

/// tmux-safe session name (no dots, colons, or whitespace).
fn sanitize_session(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| {
            if c == '.' || c == ':' || c.is_whitespace() {
                '-'
            } else {
                c
            }
        })
        .collect();
    if s.is_empty() { "swarm".to_string() } else { s }
}

/// Append `entry` to the repo's .gitignore if it is not already present.
fn ensure_gitignore(repo: &Path, entry: &str) -> Result<(), String> {
    let gi = repo.join(".gitignore");
    let mut content = fs::read_to_string(&gi).unwrap_or_default();
    if content.lines().any(|l| l.trim() == entry) {
        return Ok(());
    }
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(entry);
    content.push('\n');
    fs::write(&gi, content).map_err(|e| format!("failed to update .gitignore: {e}"))
}

fn open_editor(cfg: &AppConfig, dir: &Path) {
    if cfg.editor_cmd.trim().is_empty() {
        return;
    }
    let _ = Command::new("sh")
        .arg("-c")
        .arg(&cfg.editor_cmd)
        .current_dir(dir)
        .spawn();
}

#[cfg(target_os = "macos")]
fn applescript_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// Fire a best-effort desktop notification (no-op if no notifier is available).
fn notify(title: &str, body: &str) {
    #[cfg(target_os = "macos")]
    {
        if is_on_path("terminal-notifier") {
            let _ = Command::new("terminal-notifier")
                .args(["-title", title, "-message", body])
                .status();
        } else {
            let script = format!(
                "display notification {} with title {}",
                applescript_quote(body),
                applescript_quote(title)
            );
            let _ = Command::new("osascript").args(["-e", &script]).status();
        }
    }
    #[cfg(target_os = "linux")]
    {
        if is_on_path("notify-send") {
            let _ = Command::new("notify-send").args([title, body]).status();
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (title, body);
    }
}

fn tmux(args: &[&str]) -> Result<(), String> {
    let status = Command::new("tmux").args(args).status().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            "tmux is not installed or not in PATH".to_string()
        } else {
            format!("failed to run tmux: {e}")
        }
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("tmux {} failed", args.join(" ")))
    }
}

fn tmux_output(args: &[&str]) -> Result<String, String> {
    let out = Command::new("tmux").args(args).output().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            "tmux is not installed or not in PATH".to_string()
        } else {
            format!("failed to run tmux: {e}")
        }
    })?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("no server running") || stderr.contains("no socket") {
            Ok(String::new())
        } else {
            Err(format!("tmux {} failed: {stderr}", args.join(" ")))
        }
    }
}

/// Like `tmux_output` but trims surrounding whitespace (handy for `-P` pane ids).
fn tmux_capture(args: &[&str]) -> Result<String, String> {
    Ok(tmux_output(args)?.trim().to_string())
}

fn read_prefix_key() -> Result<String, String> {
    use crossterm::event::{Event, KeyCode, KeyModifiers, read};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    if !is_interactive() {
        return Err("prefix setup requires an interactive terminal".to_string());
    }

    println!("{SKY}⌨️  Press the exact key combination you want as your Prefix:{RESET}");
    println!(
        "   {DIM}{SUBTEXT}(macOS intercepts Command — use Ctrl, Option/Alt, or a Function key){RESET}\n"
    );

    enable_raw_mode().map_err(|e| format!("failed to enable raw mode: {e}"))?;
    let tmux_key;

    loop {
        if let Ok(Event::Key(event)) = read() {
            let mut prefix_str = String::new();

            if event.modifiers.contains(KeyModifiers::CONTROL) {
                prefix_str.push_str("C-");
            }
            if event.modifiers.contains(KeyModifiers::ALT) {
                prefix_str.push_str("M-");
            }

            match event.code {
                KeyCode::Char('c')
                    if event.modifiers.contains(KeyModifiers::CONTROL)
                        && !event.modifiers.contains(KeyModifiers::ALT) =>
                {
                    disable_raw_mode().map_err(|e| format!("failed to disable raw mode: {e}"))?;
                    die("setup aborted");
                }
                KeyCode::Char(c) => {
                    if c == ' ' {
                        if prefix_str.is_empty() {
                            disable_raw_mode()
                                .map_err(|e| format!("failed to disable raw mode: {e}"))?;
                            warn("Using Space alone blocks typing spaces! Try Ctrl+Space instead.");
                            enable_raw_mode()
                                .map_err(|e| format!("failed to enable raw mode: {e}"))?;
                            continue;
                        }
                        prefix_str.push_str("Space");
                    } else {
                        prefix_str.push(c);
                    }
                }
                KeyCode::F(n) => {
                    prefix_str.push_str(&format!("F{n}"));
                }
                KeyCode::Esc => {
                    disable_raw_mode().map_err(|e| format!("failed to disable raw mode: {e}"))?;
                    die("setup aborted");
                }
                _ => continue,
            }

            tmux_key = prefix_str;
            break;
        }
    }
    disable_raw_mode().map_err(|e| format!("failed to disable raw mode: {e}"))?;
    success(&format!("Detected Prefix: {tmux_key}"));
    Ok(tmux_key)
}

fn save_config(cfg: &AppConfig, dir: &Path) -> Result<(), String> {
    let config_file = dir.join("config.json");
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| format!("failed to serialize config: {e}"))?;
    fs::write(&config_file, json).map_err(|e| format!("failed to write config: {e}"))
}

fn load_config() -> Result<AppConfig, String> {
    let dir = config_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create config dir: {e}"))?;
    let config_file = dir.join("config.json");

    let legacy_prefix = dir.join("prefix.txt");
    if legacy_prefix.exists() {
        let p = fs::read_to_string(&legacy_prefix)
            .unwrap_or_else(|_| "C-a".to_string())
            .trim()
            .to_string();
        let cfg = AppConfig {
            prefix: p,
            ..AppConfig::default()
        };
        save_config(&cfg, &dir)?;
        fs::remove_file(legacy_prefix).ok();
        return Ok(cfg);
    }

    if config_file.exists() {
        let content =
            fs::read_to_string(&config_file).map_err(|e| format!("failed to read config: {e}"))?;
        match serde_json::from_str::<AppConfig>(&content) {
            Ok(cfg) => Ok(cfg),
            Err(e) => {
                eprintln!(
                    "Warning: invalid ~/.config/tmx/config.json ({e}). Using defaults. Run `tmx config` to fix."
                );
                Ok(AppConfig::default())
            }
        }
    } else if is_interactive() {
        print_setup_banner();
        let prefix = read_prefix_key()?;

        let editor_cmd: String = Input::with_theme(&tmx_theme())
            .with_prompt(
                "Default editor command (e.g., 'cursor .', 'code .', 'vim', or leave empty)",
            )
            .default(String::new())
            .allow_empty(true)
            .interact()
            .map_err(|e| format!("editor prompt failed: {e}"))?;

        let env_cmd: String = Input::with_theme(&tmx_theme())
            .with_prompt("Default environment activation command (e.g., 'source .venv/bin/activate', or leave empty)")
            .default(String::new())
            .allow_empty(true)
            .interact()
            .map_err(|e| format!("environment prompt failed: {e}"))?;

        let home = home_dir()?;
        let default_workspace = home.join("Projects").to_string_lossy().to_string();
        let workspace_root: String = Input::with_theme(&tmx_theme())
            .with_prompt("Workspace root directory (projects are created as <root>/<name>)")
            .default(default_workspace)
            .allow_empty(true)
            .interact()
            .map_err(|e| format!("workspace prompt failed: {e}"))?;

        let agents = detect_agents();
        if agents.is_empty() {
            info("No agent CLIs detected on your PATH yet — add them later in `tmx config`.");
        } else {
            success(&format!(
                "Detected {} agent CLI(s): {}",
                agents.len(),
                agents.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        let default_agent = choose_default_agent(&agents)?;

        let cfg = AppConfig {
            prefix,
            editor_cmd,
            env_cmd,
            workspace_root,
            agents,
            default_agent,
        };
        save_config(&cfg, &dir)?;
        Ok(cfg)
    } else {
        Err("no config found — run `tmx` in a terminal for initial setup".to_string())
    }
}

fn tab_color_for_index(idx: usize) -> &'static str {
    TAB_COLORS[idx % TAB_COLORS.len()]
}

fn style_window_tab(
    session: &str,
    window_index: u32,
    color: &str,
    color_panes: bool,
) -> Result<(), String> {
    let target = format!("{session}:{window_index}");
    tmux(&["set-window-option", "-t", &target, "@tab_color", color])?;
    tmux(&[
        "set-window-option",
        "-t",
        &target,
        "window-style",
        &format!("bg={color}"),
    ])?;
    if color_panes {
        let panes = tmux_output(&["list-panes", "-t", &target, "-F", "#{pane_id}"])?;
        for pid in panes.lines().filter(|p| !p.is_empty()) {
            tmux(&[
                "select-pane",
                "-t",
                pid,
                "-P",
                &format!("bg={color},border=fg={color}"),
            ])?;
            run_pane_script(pid, &ansi_bg_fill_shell(color))?;
        }
    }
    Ok(())
}

fn write_window_color_script(dir: &Path) -> Result<String, String> {
    let path = dir.join("tmx-window-color.sh");
    let script = format!("#!/bin/sh\n{}", window_color_script_body(true));
    fs::write(&path, script).map_err(|e| format!("failed to write window color script: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("failed to chmod window color script: {e}"))?;
    }
    Ok(path.to_string_lossy().to_string())
}

fn write_window_color_select_script(dir: &Path) -> Result<String, String> {
    let path = dir.join("tmx-window-color-select.sh");
    let script = format!("#!/bin/sh\n{}", window_color_script_body(false));
    fs::write(&path, script).map_err(|e| format!("failed to write window color select script: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("failed to chmod window color select script: {e}"))?;
    }
    Ok(path.to_string_lossy().to_string())
}

fn render_tmx_conf(cfg: &AppConfig) -> Result<String, String> {
    let dir = config_dir()?;
    let color_script = write_window_color_script(&dir)?;
    let color_select_script = write_window_color_select_script(&dir)?;
    let config_path = dir.join("tmx.conf");
    let conf_content = TMX_CONF
        .replace("{{PREFIX}}", &cfg.prefix)
        .replace("{{TMX_BIN}}", &tmx_bin())
        .replace("{{SHELL}}", &shell_cmd())
        .replace("{{CLIPBOARD_CMD}}", &clipboard_cmd())
        .replace("{{WINDOW_COLOR_SCRIPT}}", &color_script)
        .replace("{{WINDOW_COLOR_SELECT_SCRIPT}}", &color_select_script);
    fs::write(&config_path, &conf_content).map_err(|e| format!("failed to write tmx.conf: {e}"))?;
    Ok(config_path.to_string_lossy().to_string())
}

fn get_sessions() -> Result<Vec<String>, String> {
    let out = Command::new("tmux").arg("ls").output().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            "tmux is not installed or not in PATH".to_string()
        } else {
            format!("failed to run tmux: {e}")
        }
    })?;

    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        if stderr.contains("no server running") || stderr.contains("no socket") {
            return Ok(vec![]);
        }
        return Err(format!("tmux ls failed: {stderr}"));
    }

    let stdout = String::from_utf8_lossy(&out.stdout);
    Ok(stdout
        .lines()
        .filter_map(|l| l.split(':').next().map(|s| s.to_string()))
        .filter(|s| !s.is_empty())
        .collect())
}

fn select_session(prompt: &str, sessions: &[String]) -> Result<String, String> {
    if !is_interactive() {
        return Err("session selection requires an interactive terminal".to_string());
    }
    let selection = Select::with_theme(&tmx_theme())
        .with_prompt(prompt)
        .default(0)
        .items(sessions)
        .interact()
        .map_err(|e| format!("session selection failed: {e}"))?;
    Ok(sessions[selection].clone())
}

fn resolve_session_name(
    session: Option<String>,
    sessions: &[String],
    prompt: &str,
) -> Result<String, String> {
    if let Some(name) = session {
        if sessions.contains(&name) {
            Ok(name)
        } else {
            Err(format!("session '{name}' not found"))
        }
    } else {
        select_session(prompt, sessions)
    }
}

fn attach_to_session(config_path: &str, name: &str, cwd: Option<&Path>) -> Result<(), String> {
    if std::env::var("TMUX").is_ok() {
        tmux(&["switch-client", "-t", name])
    } else {
        let mut cmd = Command::new("tmux");
        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }
        let status = cmd
            .args(["-f", config_path, "attach", "-t", name])
            .spawn()
            .map_err(|e| format!("failed to attach to session '{name}': {e}"))?
            .wait()
            .map_err(|e| format!("attach to session '{name}' failed: {e}"))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("tmux attach to session '{name}' failed"))
        }
    }
}

fn shell_quote_single(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn send_cmd(pane: &str, cmd: &str) -> Result<(), String> {
    let cmd = cmd.trim_end().trim_end_matches(';').trim_end();
    if cmd.is_empty() {
        return Ok(());
    }
    // Unique buffer per pane so concurrent setup never clobbers a paste mid-flight.
    let buf = format!(
        "tmxcmd{}",
        pane.replace('%', "p").replace('.', "_")
    );
    let mut child = Command::new("tmux")
        .args(["load-buffer", "-b", &buf, "-"])
        .stdin(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("failed to start tmux load-buffer: {e}"))?;
    {
        use std::io::Write;
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "load-buffer stdin unavailable".to_string())?;
        stdin
            .write_all(cmd.as_bytes())
            .map_err(|e| format!("failed to write pane command: {e}"))?;
    }
    let status = child
        .wait()
        .map_err(|e| format!("tmux load-buffer failed: {e}"))?;
    if !status.success() {
        return Err("tmux load-buffer failed".to_string());
    }
    tmux(&["paste-buffer", "-t", pane, "-d", "-b", &buf])?;
    tmux(&["send-keys", "-t", pane, "C-m"])
}

fn pane_script_id(pane: &str) -> String {
    pane.chars()
        .map(|c| match c {
            '%' => 'p',
            ':' | '.' | '/' | '\\' => '_',
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' => c,
            _ => '_',
        })
        .collect()
}

/// Run a multi-statement shell script in a pane via a temp file (avoids paste races / line limits).
fn run_pane_script(pane: &str, script: &str) -> Result<(), String> {
    let body = script.trim().trim_end_matches(';').trim();
    if body.is_empty() {
        return Ok(());
    }
    let dir = config_dir()?.join("pane-scripts");
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create pane-scripts dir: {e}"))?;
    let path = dir.join(format!("{}.sh", pane_script_id(pane)));
    let content = format!("#!/bin/sh\n{body}\nrm -f \"$0\"\n");
    fs::write(&path, &content).map_err(|e| format!("failed to write pane script: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .map_err(|e| format!("failed to chmod pane script: {e}"))?;
    }
    // Drop any partial paste left by async tmux hooks before we run setup.
    tmux(&["send-keys", "-t", pane, "C-c"])?;
    send_cmd(pane, &format!("sh {}", shell_quote_single(&path.to_string_lossy())))
}

fn apply_pane_style(pane: &str, bg: &str) -> Result<(), String> {
    tmux(&[
        "select-pane",
        "-t",
        pane,
        "-P",
        &format!("bg={bg},border=fg={bg}"),
    ])
}

fn style_pane(pane: &str, bg: &str) -> Result<(), String> {
    apply_pane_style(pane, bg)?;
    run_pane_script(pane, &ansi_bg_fill_shell(bg))
}

fn setup_pane(
    pane: &str,
    title: &str,
    bg: &str,
    splash_cmd: &str,
    env_cmd: &str,
) -> Result<(), String> {
    apply_pane_style(pane, bg)?;
    tmux(&["select-pane", "-t", pane, "-T", title])?;
    let mut script = splash_cmd.trim_end().trim_end_matches(';').to_string();
    let extra = env_cmd.trim();
    if !extra.is_empty() {
        script.push('\n');
        script.push_str(extra);
    }
    run_pane_script(pane, &script)
}

fn setup_swarm_pane(pane: &str, index: usize, env_cmd: &str) -> Result<(), String> {
    let codename = ["ALPHA", "BRAVO", "CHARLIE", "DELTA"][index.min(3)];
    setup_pane(
        pane,
        codename,
        swarm_pane_bg(index),
        &swarm_pane_splash_script(index),
        env_cmd,
    )
}

fn list_window_pane_ids(session: &str, window: u32) -> Result<Vec<String>, String> {
    let target = format!("{session}:{window}");
    let out = tmux_output(&["list-panes", "-t", &target, "-F", "#{pane_index}|#{pane_id}"])?;
    let mut panes: Vec<(u32, String)> = out
        .lines()
        .filter_map(|line| {
            let (idx, pid) = line.split_once('|')?;
            Some((idx.parse().ok()?, pid.to_string()))
        })
        .collect();
    panes.sort_by_key(|(idx, _)| *idx);
    Ok(panes.into_iter().map(|(_, pid)| pid).collect())
}

fn setup_dev_pane(pane: &str, index: usize, env_cmd: &str, editor_cmd: &str) -> Result<(), String> {
    let splash = dev_pane_splash(index);
    let extra = if index == 0 { editor_cmd } else { env_cmd };
    setup_pane(
        pane,
        splash.title,
        splash.bg,
        &pane_splash_command(splash),
        extra,
    )
}

fn pick_layout() -> Result<usize, String> {
    let layouts = [
        "🧑‍💻  Dev Mode      — Code + Server + Shell",
        "🐝  Swarm Mode    — 4-Pane Agent Grid",
        "👁️   Observer Mode — Shell + Live Monitor",
    ];
    if is_interactive() {
        heading("Pick your workspace layout");
        let idx = Select::with_theme(&tmx_theme())
            .with_prompt("Select Layout Type")
            .default(0)
            .items(layouts)
            .interact()
            .map_err(|e| format!("layout selection failed: {e}"))?;
        print_layout_preview(idx);
        Ok(idx)
    } else {
        warn("Defaulting to Dev Mode layout (non-interactive).");
        Ok(0)
    }
}

fn run_agent_workspace(
    config_path: &str,
    cfg: &AppConfig,
    name_opt: Option<String>,
    no_attach: bool,
) -> Result<(), String> {
    let name = match name_opt {
        Some(n) => n,
        None if is_interactive() => Input::<String>::with_theme(&tmx_theme())
            .with_prompt("Enter project name")
            .interact()
            .map_err(|e| format!("project name prompt failed: {e}"))?,
        None => die("project name required in non-interactive mode"),
    };

    let sessions = get_sessions()?;
    if sessions.contains(&name) {
        attach_existing(&name);
        if no_attach || !is_interactive() {
            info(&format!(
                "Session '{name}' is running. Run `tmx attach -s {name}` to connect."
            ));
            return Ok(());
        }
        return attach_to_session(config_path, &name, None);
    }

    let workspace_root = resolve_workspace_root(cfg)?;
    let workspace_dir = workspace_root.join(&name);
    if !workspace_dir.exists() {
        info(&format!(
            "Creating new project directory at {BOLD}{}{RESET}...",
            workspace_dir.display()
        ));
        fs::create_dir_all(&workspace_dir)
            .map_err(|e| format!("failed to create project directory: {e}"))?;
    }

    let layout_idx = pick_layout()?;

    if is_interactive() && !no_attach {
        spawn_ritual(&name);
    } else {
        info(&format!("Spawning workspace '{name}'..."));
    }
    let wd = workspace_dir.to_string_lossy().to_string();
    Command::new("tmux")
        .current_dir(&workspace_dir)
        .args(["-f", config_path, "new-session", "-d", "-s", &name])
        .status()
        .map_err(|e| format!("failed to start tmux session: {e}"))?
        .success()
        .then_some(())
        .ok_or_else(|| "failed to start tmux session".to_string())?;

    if layout_idx == 1 {
        tmux(&[
            "set-window-option",
            "-t",
            &format!("{name}:1"),
            "@swarm_panes",
            "true",
        ])?;
    }

    let guard = SessionGuard::new(&name);
    let base_pane = format!("{name}:1.1");

    let setup_result = (|| -> Result<(), String> {
        match layout_idx {
            0 => {
                tmux(&[
                    "split-window",
                    "-h",
                    "-c",
                    &wd,
                    "-l",
                    "40%",
                    "-t",
                    &format!("{name}:1"),
                ])?;
                tmux(&[
                    "split-window",
                    "-v",
                    "-c",
                    &wd,
                    "-t",
                    &format!("{name}:1.2"),
                ])?;
                setup_dev_pane(&base_pane, 0, &cfg.env_cmd, &cfg.editor_cmd)?;
                setup_dev_pane(&format!("{name}:1.2"), 1, &cfg.env_cmd, "")?;
                setup_dev_pane(&format!("{name}:1.3"), 2, &cfg.env_cmd, "")?;
            }
            1 => {
                tmux(&["split-window", "-h", "-c", &wd, "-t", &format!("{name}:1")])?;
                tmux(&[
                    "split-window",
                    "-v",
                    "-c",
                    &wd,
                    "-t",
                    &format!("{name}:1.1"),
                ])?;
                tmux(&[
                    "split-window",
                    "-v",
                    "-c",
                    &wd,
                    "-t",
                    &format!("{name}:1.3"),
                ])?;
                let panes = list_window_pane_ids(&name, 1)?;
                if panes.len() < 4 {
                    return Err("swarm layout did not create four panes".to_string());
                }
                for (i, pane) in panes.iter().take(4).enumerate() {
                    setup_swarm_pane(pane, i, &cfg.env_cmd)?;
                }
            }
            2 => {
                tmux(&[
                    "split-window",
                    "-h",
                    "-c",
                    &wd,
                    "-l",
                    "30%",
                    "-t",
                    &format!("{name}:1"),
                    "sh",
                    "-c",
                    "command -v htop >/dev/null 2>&1 && exec htop || exec top",
                ])?;
                setup_pane(
                    &base_pane,
                    OBSERVER_SHELL.title,
                    OBSERVER_SHELL.bg,
                    &pane_splash_command(&OBSERVER_SHELL),
                    &cfg.env_cmd,
                )?;
                style_pane(&format!("{name}:1.2"), OBSERVER_MONITOR.bg)?;
                tmux(&[
                    "select-pane",
                    "-t",
                    &format!("{name}:1.2"),
                    "-T",
                    OBSERVER_MONITOR.title,
                ])?;
            }
            _ => {}
        }
        let tab_color = tab_color_for_index(0);
        style_window_tab(&name, 1, tab_color, layout_idx != 1)?;
        Ok(())
    })();

    if let Err(e) = setup_result {
        return Err(format!("{e} (session rolled back)"));
    }

    let skip_attach = no_attach || !is_interactive();
    if skip_attach {
        guard.keep();
        session_created(&name);
        return Ok(());
    }

    match attach_to_session(config_path, &name, Some(&workspace_dir)) {
        Ok(()) => {
            guard.keep();
            Ok(())
        }
        Err(e) => Err(format!("{e} (session rolled back)")),
    }
}

fn run_switch(config_path: &str, session: Option<String>) -> Result<(), String> {
    let sessions = get_sessions()?;
    if sessions.is_empty() {
        no_sessions();
        return Ok(());
    }

    if std::env::var("TMX_SWITCH_POPUP").is_ok() && is_interactive() {
        print_switch_banner();
    }

    let name = resolve_session_name(session, &sessions, "Switch to session")?;
    attach_to_session(config_path, &name, None)
}

fn run_attach(config_path: &str, session: Option<String>) -> Result<(), String> {
    let sessions = get_sessions()?;
    if sessions.is_empty() {
        no_sessions();
        return Ok(());
    }

    let name = if session.is_some() {
        resolve_session_name(session, &sessions, "Attach to session")?
    } else if sessions.len() == 1 {
        sessions[0].clone()
    } else {
        resolve_session_name(None, &sessions, "Attach to session")?
    };

    if !can_attach() {
        return Err(
            "attach requires an interactive terminal — session is running detached".to_string(),
        );
    }

    attach_to_session(config_path, &name, None)
}

fn run_kill(session: Option<String>) -> Result<(), String> {
    let sessions = get_sessions()?;
    if sessions.is_empty() {
        no_sessions();
        return Ok(());
    }

    let name = resolve_session_name(session, &sessions, "Select session to kill")?;
    tmux(&["kill-session", "-t", &name])?;
    session_killed(&name);
    Ok(())
}

/// Substrings (lowercase) that suggest an AI agent or CLI is waiting for user input.
const INPUT_WAIT_MARKERS: &[&str] = &[
    "waiting for your",
    "waiting for input",
    "requires input",
    "requires your",
    "needs input",
    "needs your",
    "add a follow-up",
    "press enter",
    "press return",
    "(y/n)",
    "[y/n]",
    "[Y/n]",
    "(yes/no)",
    "enter your",
    "select an option",
    "choose an option",
    "what would you like",
    "run this command",
    "allow claude",
    "allow cursor",
    "approve this",
    "confirm this",
    "proceed with",
    "continue?",
    "proceed?",
    "approve?",
    "allow?",
];

const INPUT_IDLE_COMMANDS: &[&str] = &["htop", "top", "man", "less", "more"];

fn line_looks_like_input_prompt(line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return false;
    }
    let lower = trimmed.to_lowercase();
    if INPUT_WAIT_MARKERS.iter().any(|m| lower.contains(m)) {
        return true;
    }
    if trimmed.ends_with('?') && trimmed.len() > 3 {
        return true;
    }
    // Standalone chevron prompts (not UI arrows like "→ Add a follow-up")
    if (trimmed.ends_with('>') || trimmed.ends_with('❯'))
        && !trimmed.starts_with('→')
        && !trimmed.starts_with("->")
    {
        return true;
    }
    trimmed.ends_with('➜')
}

fn pane_requires_input(
    logs: &str,
    input_enabled: bool,
    cursor_y: usize,
    pane_height: usize,
    current_command: &str,
) -> bool {
    if !input_enabled {
        return false;
    }
    if INPUT_IDLE_COMMANDS
        .iter()
        .any(|cmd| current_command.eq_ignore_ascii_case(cmd))
    {
        return false;
    }

    let lines: Vec<&str> = logs.lines().collect();
    if lines.is_empty() {
        return false;
    }

    // Agent prompts (e.g. Cursor's "Add a follow-up") often sit above the status bar,
    // so scan the full visible capture rather than only the tail.
    if lines.iter().any(|line| line_looks_like_input_prompt(line)) {
        return true;
    }

    let at_input_row = cursor_y + 1 >= pane_height.saturating_sub(1);
    if !at_input_row {
        return false;
    }

    let last = lines.last().copied().unwrap_or("").trim();
    if last.is_empty() && lines.len() >= 2 {
        return line_looks_like_input_prompt(lines[lines.len() - 2]);
    }

    line_looks_like_input_prompt(last)
}

fn cleanup_worktrees(repo: &Path, created: &[(PathBuf, String)]) {
    for (path, branch) in created {
        let _ = git::worktree_remove(repo, path);
        let _ = git::delete_branch(repo, branch);
    }
}

struct SwarmSlot {
    session_running: bool,
    trees_root: PathBuf,
    branches: Vec<String>,
    in_state: bool,
}

fn swarm_branch_prefix(name: &str) -> String {
    format!("tmx/{name}/")
}

fn inspect_swarm_slot(repo: &Path, name: &str, st: &state::State, sessions: &[String]) -> SwarmSlot {
    let prefix = swarm_branch_prefix(name);
    SwarmSlot {
        session_running: sessions.iter().any(|s| s == name),
        trees_root: repo.join(".tmx-trees").join(name),
        branches: git::list_branches_with_prefix(repo, &prefix).unwrap_or_default(),
        in_state: st.swarms.contains_key(name),
    }
}

fn cleanup_swarm_slot(repo: &Path, name: &str, state_file: &Path) -> Result<(), String> {
    let trees_root = repo.join(".tmx-trees").join(name);
    for codename in CODENAMES {
        let path = trees_root.join(codename);
        if path.exists() {
            let _ = git::worktree_remove(repo, &path);
        }
        let branch = format!("tmx/{name}/{codename}");
        if git::branch_exists(repo, &branch) {
            let _ = git::delete_branch(repo, &branch);
        }
    }
    for branch in git::list_branches_with_prefix(repo, &swarm_branch_prefix(name))? {
        let _ = git::delete_branch(repo, &branch);
    }
    if trees_root.exists() {
        fs::remove_dir_all(&trees_root)
            .map_err(|e| format!("failed to remove {}: {e}", trees_root.display()))?;
    }
    let mut st = state::State::load(state_file);
    if st.swarms.remove(name).is_some() {
        st.save(state_file)?;
    }
    Ok(())
}

/// Pick a swarm session name, cleaning stale leftovers or bumping the suffix when needed.
fn resolve_swarm_name(repo: &Path, state_file: &Path) -> Result<String, String> {
    let base_name = sanitize_session(
        repo.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("swarm"),
    );
    let sessions = get_sessions()?;
    let st = state::State::load(state_file);
    let mut name = base_name.clone();
    let mut suffix = 2;

    loop {
        let slot = inspect_swarm_slot(repo, &name, &st, &sessions);
        let has_artifacts = slot.trees_root.exists() || !slot.branches.is_empty();

        if slot.session_running {
            name = format!("{base_name}-{suffix}");
            suffix += 1;
            continue;
        }

        if !has_artifacts {
            return Ok(name);
        }

        if slot.in_state {
            if is_interactive() {
                let items = [
                    format!("Review saved work first (`tmx review -s {name}`)"),
                    format!("Start fresh — discard old work for '{name}'"),
                    format!("Use a new name ({base_name}-{suffix})"),
                ];
                let choice = Select::with_theme(&tmx_theme())
                    .with_prompt(format!(
                        "Previous swarm '{name}' left worktrees/branches but is not running"
                    ))
                    .items(&items)
                    .default(0)
                    .interact()
                    .map_err(|e| format!("swarm conflict prompt failed: {e}"))?;
                match choice {
                    0 => {
                        return Err(format!(
                            "run `tmx review -s {name}` to merge or discard the previous swarm first"
                        ));
                    }
                    1 => {
                        cleanup_swarm_slot(repo, &name, state_file)?;
                        info(&format!("Cleaned up previous swarm '{name}'."));
                        return Ok(name);
                    }
                    _ => {
                        name = format!("{base_name}-{suffix}");
                        suffix += 1;
                        continue;
                    }
                }
            }
            return Err(format!(
                "swarm '{name}' already has saved work — run `tmx review -s {name}` first"
            ));
        }

        // Orphaned branches/worktrees from a failed or partial run — safe to reclaim the name.
        if is_interactive() {
            let reclaim = Confirm::with_theme(&tmx_theme())
                .with_prompt(format!(
                    "Found leftover swarm files for '{name}' (no active session). Remove and start fresh?"
                ))
                .default(true)
                .interact()
                .map_err(|e| format!("cleanup confirm failed: {e}"))?;
            if !reclaim {
                name = format!("{base_name}-{suffix}");
                suffix += 1;
                continue;
            }
        }
        cleanup_swarm_slot(repo, &name, state_file)?;
        if is_interactive() {
            info(&format!("Cleaned up leftover swarm artifacts for '{name}'."));
        }
        return Ok(name);
    }
}

/// Remove every worktree + branch belonging to a swarm and its trees directory.
fn teardown_swarm(repo: &Path, swarm: &state::Swarm) {
    for a in &swarm.agents {
        let _ = git::worktree_remove(repo, &PathBuf::from(&a.worktree));
        let _ = git::delete_branch(repo, &a.branch);
    }
    if let Some(first) = swarm.agents.first()
        && let Some(dir) = PathBuf::from(&first.worktree).parent()
    {
        let _ = fs::remove_dir_all(dir);
    }
}

fn run_swarm(
    config_path: &str,
    cfg: &AppConfig,
    task_parts: Vec<String>,
    agents_opt: Option<usize>,
    no_attach: bool,
    state_file: &Path,
) -> Result<(), String> {
    if !git::git_available() {
        return Err("git is required for swarm mode — install git and retry".to_string());
    }
    if is_interactive() {
        print_swarm_banner();
    }

    let cwd = std::env::current_dir().map_err(|e| format!("cannot read current directory: {e}"))?;

    // Resolve repo root, offering to initialize one when needed.
    let repo = if git::is_repo(&cwd) {
        git::repo_root(&cwd)?
    } else if is_interactive() {
        warn("This folder is not a git repository.");
        let init = Confirm::with_theme(&tmx_theme())
            .with_prompt(format!("Initialize a new git repo in {}?", cwd.display()))
            .default(true)
            .interact()
            .map_err(|e| format!("confirm failed: {e}"))?;
        if !init {
            return Err("swarm needs a git repository — cd into one or allow init".to_string());
        }
        git::init_repo(&cwd)?;
        success("Initialized a fresh git repo.");
        cwd.clone()
    } else {
        return Err("not a git repository — run `git init` here first".to_string());
    };

    git::ensure_baseline_commit(&repo)?;
    let base_branch = git::current_branch(&repo)?;

    // Task prompt.
    let mut task = task_parts.join(" ").trim().to_string();
    if task.is_empty() && is_interactive() {
        task = Input::<String>::with_theme(&tmx_theme())
            .with_prompt("What should the agents build? (leave empty to start them blank)")
            .default(String::new())
            .allow_empty(true)
            .interact()
            .map_err(|e| format!("task prompt failed: {e}"))?
            .trim()
            .to_string();
    }

    let agent_cmd = resolve_agent_cmd(cfg)?;

    let count = match agents_opt {
        Some(n) => n.clamp(2, 4),
        None if is_interactive() => {
            let items = ["2 agents", "3 agents", "4 agents"];
            let idx = Select::with_theme(&tmx_theme())
                .with_prompt("How many parallel approaches?")
                .default(1)
                .items(items)
                .interact()
                .map_err(|e| format!("count selection failed: {e}"))?;
            idx + 2
        }
        None => 3,
    };

    // Session name from the repo folder; reclaim stale worktrees/branches or bump suffix.
    let name = resolve_swarm_name(&repo, state_file)?;

    ensure_gitignore(&repo, ".tmx-trees/")?;
    let trees_root = repo.join(".tmx-trees").join(&name);
    fs::create_dir_all(&trees_root).map_err(|e| format!("failed to create worktree root: {e}"))?;

    if is_interactive() && !no_attach {
        spawn_ritual(&name);
    } else {
        info(&format!("Spawning swarm '{name}' ({count} agents)..."));
    }

    // Create one worktree + branch per agent (rolled back on any failure).
    let mut created: Vec<(PathBuf, String)> = Vec::new();
    for codename in CODENAMES.iter().take(count) {
        let branch = format!("tmx/{name}/{codename}");
        let path = trees_root.join(codename);
        if let Err(e) = git::worktree_add(&repo, &path, &branch, &base_branch) {
            cleanup_worktrees(&repo, &created);
            let _ = fs::remove_dir_all(&trees_root);
            return Err(format!("{e} (rolled back)"));
        }
        created.push((path, branch));
    }

    // Start the session with the first pane inside the first worktree.
    let first_str = trees_root.join(CODENAMES[0]).to_string_lossy().to_string();
    let started = Command::new("tmux")
        .args([
            "-f",
            config_path,
            "new-session",
            "-d",
            "-s",
            &name,
            "-c",
            &first_str,
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !started {
        cleanup_worktrees(&repo, &created);
        let _ = fs::remove_dir_all(&trees_root);
        return Err("failed to start tmux session (rolled back)".to_string());
    }

    let guard = SessionGuard::new(&name);

    let mut agent_entries: Vec<state::AgentEntry> = Vec::new();
    let wire = (|| -> Result<(), String> {
        let base_pane =
            tmux_capture(&["list-panes", "-t", &format!("{name}:1"), "-F", "#{pane_id}"])?
                .lines()
                .next()
                .unwrap_or("")
                .to_string();
        if base_pane.is_empty() {
            return Err("could not locate base pane".to_string());
        }
        tmux(&[
            "set-window-option",
            "-t",
            &format!("{name}:1"),
            "@swarm_panes",
            "true",
        ])?;
        let mut panes = vec![base_pane];
        for codename in CODENAMES.iter().take(count).skip(1) {
            let path = trees_root.join(codename).to_string_lossy().to_string();
            let pid = tmux_capture(&[
                "split-window",
                "-P",
                "-F",
                "#{pane_id}",
                "-t",
                &format!("{name}:1"),
                "-c",
                &path,
            ])?;
            panes.push(pid);
        }
        tmux(&["select-layout", "-t", &format!("{name}:1"), "tiled"])?;

        let invocation = agent_invocation(&agent_cmd, &task);
        for (i, pane) in panes.iter().enumerate() {
            setup_swarm_pane(pane, i, &invocation)?;
            agent_entries.push(state::AgentEntry {
                codename: CODENAMES[i].to_string(),
                branch: format!("tmx/{name}/{}", CODENAMES[i]),
                worktree: trees_root.join(CODENAMES[i]).to_string_lossy().to_string(),
                pane: pane.clone(),
            });
        }

        style_window_tab(&name, 1, tab_color_for_index(0), false)?;
        Ok(())
    })();

    if let Err(e) = wire {
        cleanup_worktrees(&repo, &created);
        let _ = fs::remove_dir_all(&trees_root);
        return Err(format!("{e} (rolled back)"));
    }

    // Persist so `tmx review` can find the worktrees later.
    let mut st = state::State::load(state_file);
    st.swarms.insert(
        name.clone(),
        state::Swarm {
            session: name.clone(),
            repo_root: repo.to_string_lossy().to_string(),
            base_branch: base_branch.clone(),
            task: task.clone(),
            agents: agent_entries,
        },
    );
    st.save(state_file)?;

    if no_attach || !is_interactive() {
        guard.keep();
        session_created(&name);
        highlight(&format!("Review results later: tmx review -s {name}"));
        return Ok(());
    }

    match attach_to_session(config_path, &name, Some(&repo)) {
        Ok(()) => {
            guard.keep();
            Ok(())
        }
        Err(e) => {
            cleanup_worktrees(&repo, &created);
            let _ = fs::remove_dir_all(&trees_root);
            let mut st = state::State::load(state_file);
            st.swarms.remove(&name);
            let _ = st.save(state_file);
            Err(format!("{e} (rolled back)"))
        }
    }
}

fn run_review(session: Option<String>, state_file: &Path, cfg: &AppConfig) -> Result<(), String> {
    let mut st = state::State::load(state_file);
    if st.swarms.is_empty() {
        info("No swarms found. Spawn one with `tmx swarm \"your task\"`.");
        return Ok(());
    }

    let names: Vec<String> = st.swarms.keys().cloned().collect();
    let session = match session {
        Some(s) => {
            if !st.swarms.contains_key(&s) {
                return Err(format!("no swarm named '{s}'"));
            }
            s
        }
        None if names.len() == 1 => names[0].clone(),
        None => {
            if !is_interactive() {
                return Err("multiple swarms found — pass -s <session>".to_string());
            }
            select_session("Review which swarm?", &names)?
        }
    };

    let swarm = st.swarms.get(&session).cloned().ok_or("swarm not found")?;
    let repo = PathBuf::from(&swarm.repo_root);

    print_review_banner();
    println!(
        "  {DIM}Task:{RESET} {}\n",
        if swarm.task.is_empty() {
            "(none)".to_string()
        } else {
            swarm.task.clone()
        }
    );

    let mut labels: Vec<String> = Vec::new();
    for (i, a) in swarm.agents.iter().enumerate() {
        let stat = git::working_diff_stat(&PathBuf::from(&a.worktree), &swarm.base_branch)
            .unwrap_or(git::DiffStat {
                files: 0,
                insertions: 0,
                deletions: 0,
                untracked: 0,
            });
        review_agent_card(
            &a.codename.to_uppercase(),
            swarm_pane_bg(i),
            stat.files,
            stat.insertions,
            stat.deletions,
            stat.untracked,
        );
        labels.push(format!(
            "Keep {} ({} file{}, +{}/-{})",
            a.codename.to_uppercase(),
            stat.files + stat.untracked,
            if stat.files + stat.untracked == 1 {
                ""
            } else {
                "s"
            },
            stat.insertions,
            stat.deletions
        ));
    }
    println!();

    if !is_interactive() {
        info("Run `tmx review` in a terminal to keep or discard interactively.");
        return Ok(());
    }

    let mut options = labels.clone();
    options.push("🗑️  Discard all & clean up".to_string());
    options.push("Cancel".to_string());
    let cancel_idx = options.len() - 1;
    let discard_idx = options.len() - 2;

    let choice = Select::with_theme(&tmx_theme())
        .with_prompt("Which approach do you want to keep?")
        .default(0)
        .items(&options)
        .interact()
        .map_err(|e| format!("review selection failed: {e}"))?;

    if choice == cancel_idx {
        info("No changes made. Agents are still running.");
        return Ok(());
    }

    if choice == discard_idx {
        let confirm = Confirm::with_theme(&tmx_theme())
            .with_prompt("Discard ALL agent work and remove their worktrees?")
            .default(false)
            .interact()
            .map_err(|e| format!("confirm failed: {e}"))?;
        if !confirm {
            info("Cancelled.");
            return Ok(());
        }
        teardown_swarm(&repo, &swarm);
        st.swarms.remove(&session);
        st.save(state_file)?;
        let _ = tmux(&["kill-session", "-t", &session]);
        session_killed(&session);
        return Ok(());
    }

    // Keep the chosen agent.
    let chosen = &swarm.agents[choice];
    let worktree = PathBuf::from(&chosen.worktree);

    if git::is_dirty(&repo)? {
        return Err(format!(
            "your main working tree ({}) has uncommitted changes — commit or stash them first",
            repo.display()
        ));
    }

    if git::is_dirty(&worktree)? {
        let msg = if swarm.task.is_empty() {
            format!("tmx: {} agent work", chosen.codename)
        } else {
            format!("tmx: {} — {}", chosen.codename, swarm.task)
        };
        git::commit_all(&worktree, &msg)?;
    }

    git::checkout(&repo, &swarm.base_branch)?;
    info(&format!(
        "Merging {} into {}...",
        chosen.codename.to_uppercase(),
        swarm.base_branch
    ));
    match git::merge_branch(&repo, &chosen.branch)? {
        git::MergeOutcome::Clean => {
            success(&format!(
                "Merged {} into {} 🎉",
                chosen.codename.to_uppercase(),
                swarm.base_branch
            ));
        }
        git::MergeOutcome::Conflict(files) => {
            warn("Merge hit conflicts in these files:");
            for f in &files {
                println!("    {YELLOW}• {f}{RESET}");
            }
            println!();
            info(
                "Resolve them, then run `git commit` to finish — or `git merge --abort` to cancel.",
            );
            if !cfg.editor_cmd.trim().is_empty() {
                let open = Confirm::with_theme(&tmx_theme())
                    .with_prompt(format!("Open {} to resolve now?", repo.display()))
                    .default(true)
                    .interact()
                    .map_err(|e| format!("confirm failed: {e}"))?;
                if open {
                    open_editor(cfg, &repo);
                }
            }
            return Ok(());
        }
    }

    let cleanup = Confirm::with_theme(&tmx_theme())
        .with_prompt("Clean up all agent worktrees, branches, and the tmux session?")
        .default(true)
        .interact()
        .map_err(|e| format!("confirm failed: {e}"))?;
    if cleanup {
        teardown_swarm(&repo, &swarm);
        st.swarms.remove(&session);
        st.save(state_file)?;
        let _ = tmux(&["kill-session", "-t", &session]);
        success("Swarm cleaned up. Nice work!");
    } else {
        info("Kept worktrees in place. Re-run `tmx review` anytime.");
    }
    Ok(())
}

fn run_doctor(cfg: &AppConfig) -> Result<(), String> {
    print_doctor_banner();

    match Command::new("tmux").arg("-V").output() {
        Ok(o) if o.status.success() => {
            let v = String::from_utf8_lossy(&o.stdout).trim().to_string();
            doctor_check(true, "tmux installed", &v);
        }
        _ => {
            doctor_check(false, "tmux installed", "required");
            doctor_hint("Install: brew install tmux  (macOS)  ·  apt install tmux  (Debian)");
        }
    }

    let git_ok = git::git_available();
    doctor_check(
        git_ok,
        "git installed",
        if git_ok {
            "powers swarm worktrees"
        } else {
            "required for swarm"
        },
    );
    if !git_ok {
        doctor_hint("Install git to use `tmx swarm` / `tmx review`.");
    }

    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    let truecolor = colorterm.contains("truecolor") || colorterm.contains("24bit");
    doctor_check(
        truecolor,
        "truecolor terminal",
        if truecolor {
            colorterm.as_str()
        } else {
            "backgrounds may look flat"
        },
    );
    if !truecolor {
        doctor_hint("Use Ghostty / iTerm2 / WezTerm, or set COLORTERM=truecolor.");
    }

    let clip = clipboard_cmd();
    let clip_ok = clip != "cat";
    doctor_check(
        clip_ok,
        "clipboard tool",
        if clip_ok {
            clip.as_str()
        } else {
            "yank disabled"
        },
    );
    if !clip_ok {
        doctor_hint("Install pbcopy (macOS), wl-copy (Wayland), or xclip (X11).");
    }

    println!();
    heading("AI agents on your PATH");
    let detected = detect_agents();
    for (bin, display, _tmpl) in KNOWN_AGENTS {
        let found = detected.contains_key(*bin);
        doctor_check(found, display, if found { bin } else { "not found" });
    }
    if detected.is_empty() {
        doctor_hint("No agent CLIs detected — install one (e.g. Claude Code) to power the swarm.");
    }

    println!();
    heading("Configuration");
    doctor_check(true, "prefix key", &cfg.prefix);
    let editor = if cfg.editor_cmd.is_empty() {
        "not set"
    } else {
        cfg.editor_cmd.as_str()
    };
    doctor_check(!cfg.editor_cmd.is_empty(), "editor command", editor);
    let default_agent = if cfg.default_agent.is_empty() {
        "auto".to_string()
    } else {
        cfg.default_agent.clone()
    };
    doctor_check(true, "default agent", &default_agent);
    println!();
    Ok(())
}

/// Infer an agent's live status from how its output changed between refreshes.
fn classify_agent(prev_tail: &[String], current_tail: &[String]) -> AgentState {
    let changed = prev_tail != current_tail;
    let last = current_tail
        .iter()
        .rev()
        .find(|l| !l.trim().is_empty())
        .cloned()
        .unwrap_or_default();
    let l = last.trim();
    let low = l.to_lowercase();
    let needs_input = low.ends_with('?')
        || low.contains("(y/n)")
        || low.contains("[y/n]")
        || low.contains("y/n/a")
        || low.contains("yes/no")
        || low.contains("continue?")
        || low.contains("proceed")
        || low.contains("overwrite")
        || low.contains("password")
        || low.contains("do you want")
        || low.contains("(press");

    if changed {
        return if needs_input {
            AgentState::NeedsInput
        } else {
            AgentState::Working
        };
    }
    if needs_input {
        return AgentState::NeedsInput;
    }
    let at_prompt = l.ends_with('$')
        || l.ends_with('%')
        || l.ends_with('#')
        || (l.ends_with('>') && !l.ends_with("=>"));
    if at_prompt {
        AgentState::Done
    } else {
        AgentState::Idle
    }
}

struct DashboardPane {
    session: String,
    window: String,
    pane_idx: String,
    pane_title: String,
    cmd: String,
    pane_id: String,
    logs: String,
    input_waiting: bool,
}

fn run_ps_with_options(
    prev_logs: &mut HashMap<String, Vec<String>>,
    frame: usize,
    show_stats: bool,
    highlight_diff: bool,
    mut statuses: Option<&mut HashMap<String, AgentState>>,
) -> Result<(), String> {
    let stdout = tmux_output(&[
        "list-panes",
        "-a",
        "-F",
        concat!(
            "#{session_name}|#{window_name}|#{pane_index}|#{pane_title}|#{pane_current_command}|#{pane_id}|",
            "#{?pane_input_off,0,1}|#{cursor_y}|#{pane_height}"
        ),
    ])?;

    if stdout.trim().is_empty() {
        no_sessions();
        return Ok(());
    }

    let mut sessions = HashSet::new();
    let mut pane_count = 0usize;
    let mut panes = Vec::new();

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() != 9 {
            continue;
        }
        let (session, window, pane_idx, pane_title, cmd, pane_id, input_on, cursor_y, pane_height) = (
            parts[0],
            parts[1],
            parts[2],
            parts[3],
            parts[4],
            parts[5],
            parts[6] == "1",
            parts[7].parse().unwrap_or(0),
            parts[8].parse().unwrap_or(0),
        );

        sessions.insert(session.to_string());
        pane_count += 1;

        let cap_out = tmux_output(&["capture-pane", "-p", "-t", pane_id])?;
        let logs = cap_out.trim_end().to_string();
        let input_waiting =
            pane_requires_input(&logs, input_on, cursor_y, pane_height, cmd);

        panes.push(DashboardPane {
            session: session.to_string(),
            window: window.to_string(),
            pane_idx: pane_idx.to_string(),
            pane_title: pane_title.to_string(),
            cmd: cmd.to_string(),
            pane_id: pane_id.to_string(),
            logs,
            input_waiting,
        });
    }

    let waiting_count = panes.iter().filter(|p| p.input_waiting).count();

    print_dashboard_header();
    if show_stats {
        print_dashboard_stats(sessions.len(), pane_count);
    }
    if waiting_count > 0 {
        let label = if waiting_count == 1 {
            "pane needs input"
        } else {
            "panes need input"
        };
        warn(&format!("🔔 {waiting_count} {label} — attach to respond"));
    }
    println!();

    for pane in panes {
        let current_tail: Vec<String> = if pane.logs.is_empty() {
            vec![]
        } else {
            let lines: Vec<&str> = pane.logs.lines().collect();
            let tail_start = lines.len().saturating_sub(3);
            lines[tail_start..]
                .iter()
                .map(|l| {
                    if l.len() > 86 {
                        format!("{}...", &l[..83])
                    } else {
                        (*l).to_string()
                    }
                })
                .collect()
        };
        let prev_tail = prev_logs.get(&pane.pane_id).cloned().unwrap_or_default();

        let status = if highlight_diff {
            let mut s = classify_agent(&prev_tail, &current_tail);
            if pane.input_waiting {
                s = AgentState::NeedsInput;
            }
            if let Some(map) = statuses.as_deref_mut() {
                map.insert(pane.pane_id.clone(), s);
            }
            Some(s)
        } else if pane.input_waiting {
            Some(AgentState::NeedsInput)
        } else {
            None
        };

        pane_card(
            &pane.session,
            &pane.window,
            &pane.pane_idx,
            &pane.pane_title,
            &pane.cmd,
            status,
        );
        if current_tail.is_empty() {
            pane_log_line_styled("(no output yet)", false);
        } else {
            for (i, log_line) in current_tail.iter().enumerate() {
                let changed = pane.input_waiting
                    || (highlight_diff
                        && prev_tail.get(i).map(String::as_str) != Some(log_line.as_str()));
                pane_log_line_styled(log_line, changed);
            }
        }
        if highlight_diff {
            prev_logs.insert(pane.pane_id.clone(), current_tail);
        }
        pane_card_end();
        println!();
    }

    if highlight_diff {
        monitor_footer(frame);
    }
    Ok(())
}

fn run_ps() -> Result<(), String> {
    let mut prev = HashMap::new();
    run_ps_with_options(&mut prev, 0, false, false, None)
}

/// Publish a swarm status summary to a tmux user option for the status bar badge.
fn set_swarm_badge(statuses: &HashMap<String, AgentState>) {
    if statuses.is_empty() {
        return;
    }
    let (mut working, mut done, mut needs, mut idle) = (0, 0, 0, 0);
    for s in statuses.values() {
        match s {
            AgentState::Working => working += 1,
            AgentState::Done => done += 1,
            AgentState::NeedsInput => needs += 1,
            AgentState::Idle => idle += 1,
        }
    }
    let mut parts = Vec::new();
    if working > 0 {
        parts.push(format!("{working} working"));
    }
    if needs > 0 {
        parts.push(format!("{needs} need input"));
    }
    if done > 0 {
        parts.push(format!("{done} done"));
    }
    if idle > 0 {
        parts.push(format!("{idle} idle"));
    }
    let summary = parts.join(" · ");
    let _ = Command::new("tmux")
        .args(["set-option", "-g", "@tmx_swarm", &summary])
        .status();
}

fn run_monitor() -> Result<(), String> {
    let _guard = MonitorGuard;
    let _ = write!(io::stdout(), "\x1B[?1049h\x1B[?25l");
    let mut prev_logs = HashMap::new();
    let mut prev_status: HashMap<String, AgentState> = HashMap::new();
    let mut frame = 0usize;
    loop {
        let _ = write!(io::stdout(), "\x1B[2J\x1B[1;1H");
        let mut statuses: HashMap<String, AgentState> = HashMap::new();
        run_ps_with_options(&mut prev_logs, frame, true, true, Some(&mut statuses))?;

        // Tap the user on the shoulder when an agent finishes or needs input.
        if frame > 0 {
            for (pane, st) in &statuses {
                if prev_status.get(pane).copied() != Some(*st) {
                    match st {
                        AgentState::NeedsInput => notify(
                            "🤖 Agent needs input",
                            "An agent is waiting for your response.",
                        ),
                        AgentState::Done => notify(
                            "✅ Agent finished",
                            "An agent returned to the shell prompt.",
                        ),
                        _ => {}
                    }
                }
            }
        }
        set_swarm_badge(&statuses);
        prev_status = statuses;

        frame = frame.wrapping_add(1);
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

fn print_cheatsheet(prefix: &str) {
    print_cheatsheet_header();
    println!();

    cheat_section("💡 What is a Prefix?");
    println!(
        "  {TEXT}Press your prefix, release, then press the action key — like a clutch pedal.{RESET}"
    );
    println!("  {DIM}Your prefix:{RESET} {BOLD}{MAUVE}{prefix}{RESET}");

    cheat_section("🪟 Panes");
    cheat_row(prefix, "|", "Split vertically (left / right)");
    cheat_row(prefix, "-", "Split horizontally (top / bottom)");
    cheat_row(prefix, "x", "Kill the current pane");
    cheat_row(prefix, "z", "Zoom pane fullscreen (toggle)");

    cheat_section("🧭 Navigation");
    println!(
        "  {BOLD}{SKY}Option + Arrows{RESET}  {DIM}→{RESET}  {TEXT}Switch panes instantly (no prefix!){RESET}"
    );
    cheat_row(prefix, "Arrows", "Switch panes (hold prefix, tap arrows)");

    cheat_section("📑 Windows & Sessions");
    cheat_row(prefix, "c", "Create a new window (tab)");
    cheat_row(prefix, "<number>", "Jump to window by number");
    cheat_row(prefix, "w", "⚡ Popup quick switcher between projects");

    cheat_section("🐝 Swarm (parallel agents)");
    println!(
        "  {BOLD}{SKY}tmx swarm{RESET} {DIM}\"your task\"{RESET}  {DIM}→{RESET}  {TEXT}Spawn N agents, each in its own git worktree{RESET}"
    );
    cheat_row(prefix, "r", "🔍 Review swarm & keep the best (merge)");

    cheat_section("⚡ 10x Agentic Power");
    cheat_row(prefix, "y", "📋 Yank last 200 lines to clipboard");
    cheat_row(prefix, "P", "🖨️  Toggle auto-logger to disk");
    cheat_row(prefix, "L", "🔒 Lock pane (disable keyboard input)");
    cheat_row(prefix, "U", "🔓 Unlock pane");
    cheat_row(prefix, "S", "💾 1-click crash report → tmx_crash.log");
    cheat_row(prefix, "g", "🪟 Floating scratchpad popup");
    cheat_row(prefix, "s", "🤖 Synchronize typing across ALL panes");
    cheat_row(prefix, "/", "🔍 Search scrollback backwards");
    cheat_row(prefix, "d", "Detach (session keeps running)");

    if !clipboard_available() {
        section("📋 Clipboard");
        println!(
            "  {SUBTEXT}Install pbcopy (macOS), wl-copy (Wayland), or xclip (X11) for yank.{RESET}"
        );
        section_end();
    }

    cheat_section("🖱️  Mouse Mode");
    println!(
        "  {TEXT}Click panes, tabs, and drag borders to resize — toasts teach keyboard shortcuts!{RESET}"
    );
    println!();
}

fn run_config(mut cfg: AppConfig, config_dir: &Path) -> Result<AppConfig, String> {
    if !is_interactive() {
        return Err("configuration requires an interactive terminal".to_string());
    }

    config_banner();

    cfg.prefix = read_prefix_key()?;

    cfg.editor_cmd = Input::with_theme(&tmx_theme())
        .with_prompt("Default editor command (e.g., 'cursor .')")
        .default(cfg.editor_cmd.clone())
        .allow_empty(true)
        .interact()
        .map_err(|e| format!("editor prompt failed: {e}"))?;

    cfg.env_cmd = Input::with_theme(&tmx_theme())
        .with_prompt("Environment activation command")
        .default(cfg.env_cmd.clone())
        .allow_empty(true)
        .interact()
        .map_err(|e| format!("environment prompt failed: {e}"))?;

    let home = home_dir()?;
    let default_workspace = if cfg.workspace_root.trim().is_empty() {
        home.join("Projects").to_string_lossy().to_string()
    } else {
        cfg.workspace_root.clone()
    };
    cfg.workspace_root = Input::with_theme(&tmx_theme())
        .with_prompt("Workspace root directory")
        .default(default_workspace)
        .allow_empty(true)
        .interact()
        .map_err(|e| format!("workspace prompt failed: {e}"))?;

    // Merge freshly detected agents with any custom ones the user added by hand.
    for (k, v) in detect_agents() {
        cfg.agents.entry(k).or_insert(v);
    }
    if cfg.agents.is_empty() {
        info("No agent CLIs detected — install one (e.g. Claude Code) to power swarms.");
    } else {
        success(&format!(
            "Agents available: {}",
            cfg.agents.keys().cloned().collect::<Vec<_>>().join(", ")
        ));
    }
    cfg.default_agent = choose_default_agent(&cfg.agents)?;

    save_config(&cfg, config_dir)?;
    render_tmx_conf(&cfg)?;

    success("Configuration updated successfully!");
    println!();
    Ok(cfg)
}

fn interactive_menu(
    config_path: &str,
    mut cfg: AppConfig,
    config_dir: &Path,
) -> Result<(), String> {
    if !is_interactive() {
        return Err(
            "interactive menu requires a terminal — use a subcommand instead (e.g. `tmx ps`)"
                .to_string(),
        );
    }

    let state_file = config_dir.join("state.json");

    let options = [
        "🐝  Spawn Parallel Agents (Swarm)",
        "🔍  Review Swarm & Keep the Best",
        "🚀  Create Agent Workspace",
        "🔀  Switch / Attach to Session",
        "🌐  Global Dashboard (ps)",
        "📡  Live Monitor",
        "🩺  Doctor (health check)",
        "⚙️   Change Configuration",
        "💀  Kill Session",
        "⌨️   Cheatsheet",
        "👋  Exit",
    ];

    loop {
        clear_screen();
        menu_banner();

        let selection = Select::with_theme(&tmx_theme())
            .with_prompt("Choose an action")
            .default(0)
            .items(options)
            .interact()
            .map_err(|e| format!("menu selection failed: {e}"))?;

        match selection {
            0 => run_swarm(config_path, &cfg, vec![], None, false, &state_file)?,
            1 => {
                run_review(None, &state_file, &cfg)?;
                pause_menu();
            }
            2 => run_agent_workspace(config_path, &cfg, None, false)?,
            3 => run_switch(config_path, None)?,
            4 => {
                run_ps()?;
                pause_menu();
            }
            5 => run_monitor()?,
            6 => {
                run_doctor(&cfg)?;
                pause_menu();
            }
            7 => {
                cfg = run_config(cfg, config_dir)?;
                pause_menu();
            }
            8 => {
                run_kill(None)?;
                pause_menu();
            }
            9 => {
                print_cheatsheet(&cfg.prefix);
                pause_menu();
            }
            _ => {
                print_goodbye();
                break;
            }
        }
    }
    Ok(())
}

fn main() {
    ctrlc::set_handler(move || {
        let _ = write!(io::stdout(), "\x1B[?25h");
        std::process::exit(0);
    })
    .unwrap_or(());

    let result = (|| -> Result<(), String> {
        let config_dir = config_dir()?;
        let cfg = load_config()?;
        let config_path = render_tmx_conf(&cfg)?;
        let state_file = state_path()?;
        let cli = Cli::parse();

        match cli.command {
            Some(Commands::Agent { name, no_attach }) => {
                run_agent_workspace(&config_path, &cfg, name, no_attach)
            }
            Some(Commands::Swarm {
                task,
                agents,
                no_attach,
            }) => run_swarm(&config_path, &cfg, task, agents, no_attach, &state_file),
            Some(Commands::Review { session }) => run_review(session, &state_file, &cfg),
            Some(Commands::Attach { session }) => run_attach(&config_path, session),
            Some(Commands::Switch { session }) => run_switch(&config_path, session),
            Some(Commands::Ps) => run_ps(),
            Some(Commands::Monitor) => run_monitor(),
            Some(Commands::Kill { session }) => run_kill(session),
            Some(Commands::Config) => {
                run_config(cfg, &config_dir)?;
                Ok(())
            }
            Some(Commands::Doctor) => run_doctor(&cfg),
            Some(Commands::Cheat) => {
                print_cheatsheet(&cfg.prefix);
                Ok(())
            }
            None => interactive_menu(&config_path, cfg, &config_dir),
        }
    })();

    if let Err(msg) = result {
        die(&msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_question_prompt_at_bottom() {
        let logs = "Working...\nDo you want to proceed? (y/n)\n";
        assert!(pane_requires_input(logs, true, 2, 3, "node"));
    }

    #[test]
    fn detects_follow_up_prompt() {
        let logs = "Task complete.\nAdd a follow-up\n";
        assert!(pane_requires_input(logs, true, 1, 2, "bash"));
    }

    #[test]
    fn ignores_locked_pane() {
        let logs = "Do you want to proceed? (y/n)\n";
        assert!(!pane_requires_input(logs, false, 1, 2, "bash"));
    }

    #[test]
    fn ignores_htop() {
        let logs = "Press F1 for help\n";
        assert!(!pane_requires_input(logs, true, 0, 24, "htop"));
    }

    #[test]
    fn detects_cursor_follow_up_above_status_bar() {
        let logs = "  → Add a follow-up\n\n  Composer 2.5 Fast · 24%\n  ~/Projects/foo\n";
        assert!(pane_requires_input(logs, true, 3, 4, "bash"));
    }

    #[test]
    fn detects_chevron_prompt() {
        let logs = "Ready.\n❯ ";
        assert!(pane_requires_input(logs, true, 1, 2, "bash"));
    }
}
