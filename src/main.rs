use clap::{Parser, Subcommand};
use dialoguer::{Input, Select, theme::ColorfulTheme};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

const TMX_CONF: &str = include_str!("tmx.conf");

#[derive(Serialize, Deserialize, Clone)]
struct AppConfig {
    prefix: String,
    editor_cmd: String,
    env_cmd: String,
    #[serde(default)]
    workspace_root: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            prefix: "C-a".to_string(),
            editor_cmd: String::new(),
            env_cmd: String::new(),
            workspace_root: String::new(),
        }
    }
}

#[derive(Parser)]
#[command(name = "tmx", about = "10x Agentic Tmux Workspace Wrapper", version)]
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
    /// View the Tmux cheatsheet
    Cheat,
}

struct CursorGuard;

impl Drop for CursorGuard {
    fn drop(&mut self) {
        let _ = write!(io::stdout(), "\x1B[?25h");
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

fn die(msg: &str) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(1);
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

fn read_prefix_key() -> Result<String, String> {
    use crossterm::event::{Event, KeyCode, KeyModifiers, read};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    if !is_interactive() {
        return Err("prefix setup requires an interactive terminal".to_string());
    }

    println!("⌨️  Press the exact key combination you want to use as your Prefix:");
    println!(
        "   (Note: The 'Command' key is intercepted by macOS. Please use Ctrl, Option/Alt, or a Function key.)\n"
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
                            println!(
                                "\n⚠️  WARNING: Using 'Space' alone as a prefix means you will NEVER be able to type a space character in your terminal! Please choose a combination like 'Ctrl+Space'.\n"
                            );
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
    println!("✅ Detected Prefix: {tmux_key}");
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
        println!("🚀 Welcome to TMX 10x Setup!");
        let prefix = read_prefix_key()?;

        let editor_cmd: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt(
                "Default editor command (e.g., 'cursor .', 'code .', 'vim', or leave empty)",
            )
            .default(String::new())
            .allow_empty(true)
            .interact()
            .map_err(|e| format!("editor prompt failed: {e}"))?;

        let env_cmd: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Default environment activation command (e.g., 'source .venv/bin/activate', or leave empty)")
            .default(String::new())
            .allow_empty(true)
            .interact()
            .map_err(|e| format!("environment prompt failed: {e}"))?;

        let home = home_dir()?;
        let default_workspace = home.join("Projects").to_string_lossy().to_string();
        let workspace_root: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Workspace root directory (projects are created as <root>/<name>)")
            .default(default_workspace)
            .allow_empty(true)
            .interact()
            .map_err(|e| format!("workspace prompt failed: {e}"))?;

        let cfg = AppConfig {
            prefix,
            editor_cmd,
            env_cmd,
            workspace_root,
        };
        save_config(&cfg, &dir)?;
        Ok(cfg)
    } else {
        Err("no config found — run `tmx` in a terminal for initial setup".to_string())
    }
}

fn render_tmx_conf(cfg: &AppConfig) -> Result<String, String> {
    let dir = config_dir()?;
    let config_path = dir.join("tmx.conf");
    let conf_content = TMX_CONF
        .replace("{{PREFIX}}", &cfg.prefix)
        .replace("{{TMX_BIN}}", &tmx_bin())
        .replace("{{SHELL}}", &shell_cmd())
        .replace("{{CLIPBOARD_CMD}}", &clipboard_cmd());
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
    let selection = Select::with_theme(&ColorfulTheme::default())
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

fn send_cmd(pane: &str, cmd: &str) -> Result<(), String> {
    if cmd.trim().is_empty() {
        return Ok(());
    }
    tmux(&["send-keys", "-t", pane, cmd, "C-m"])
}

fn pick_layout() -> Result<usize, String> {
    let layouts = [
        "Dev Mode (Code + Server + Shell)",
        "Swarm Mode (4-Pane Grid)",
        "Observer Mode (Logs + Monitor)",
    ];
    if is_interactive() {
        Select::with_theme(&ColorfulTheme::default())
            .with_prompt("Select Layout Type")
            .default(0)
            .items(layouts)
            .interact()
            .map_err(|e| format!("layout selection failed: {e}"))
    } else {
        eprintln!("Note: defaulting to Dev Mode layout (non-interactive).");
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
        None if is_interactive() => Input::<String>::with_theme(&ColorfulTheme::default())
            .with_prompt("Enter project name")
            .interact()
            .map_err(|e| format!("project name prompt failed: {e}"))?,
        None => die("project name required in non-interactive mode"),
    };

    let sessions = get_sessions()?;
    if sessions.contains(&name) {
        println!("Session '{name}' already exists. Attaching...");
        if no_attach || !is_interactive() {
            println!("Session '{name}' is running. Run `tmx attach -s {name}` to connect.");
            return Ok(());
        }
        return attach_to_session(config_path, &name, None);
    }

    let workspace_root = resolve_workspace_root(cfg)?;
    let workspace_dir = workspace_root.join(&name);
    if !workspace_dir.exists() {
        println!(
            "Creating new project directory at {}...",
            workspace_dir.display()
        );
        fs::create_dir_all(&workspace_dir)
            .map_err(|e| format!("failed to create project directory: {e}"))?;
    }

    let layout_idx = pick_layout()?;

    println!("Starting Workspace '{name}'...");
    let wd = workspace_dir.to_string_lossy().to_string();
    Command::new("tmux")
        .current_dir(&workspace_dir)
        .args(["-f", config_path, "new-session", "-d", "-s", &name])
        .status()
        .map_err(|e| format!("failed to start tmux session: {e}"))?
        .success()
        .then_some(())
        .ok_or_else(|| "failed to start tmux session".to_string())?;

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
                send_cmd(&base_pane, &cfg.editor_cmd)?;
                send_cmd(&format!("{name}:1.2"), &cfg.env_cmd)?;
                send_cmd(&format!("{name}:1.3"), &cfg.env_cmd)?;
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
                for i in 1..=4 {
                    send_cmd(&format!("{name}:1.{i}"), &cfg.env_cmd)?;
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
                send_cmd(&base_pane, &cfg.env_cmd)?;
            }
            _ => {}
        }
        Ok(())
    })();

    if let Err(e) = setup_result {
        return Err(format!("{e} (session rolled back)"));
    }

    let skip_attach = no_attach || !is_interactive();
    if skip_attach {
        guard.keep();
        println!("Session '{name}' created. Run `tmx attach -s {name}` to connect.");
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
        println!("No active tmux sessions.");
        return Ok(());
    }

    let name = resolve_session_name(session, &sessions, "Switch to session")?;
    attach_to_session(config_path, &name, None)
}

fn run_attach(config_path: &str, session: Option<String>) -> Result<(), String> {
    let sessions = get_sessions()?;
    if sessions.is_empty() {
        println!("No active tmux sessions.");
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
        println!("No active tmux sessions.");
        return Ok(());
    }

    let name = resolve_session_name(session, &sessions, "Select session to kill")?;
    tmux(&["kill-session", "-t", &name])?;
    println!("Session '{name}' killed.");
    Ok(())
}

fn run_ps() -> Result<(), String> {
    let stdout = tmux_output(&[
        "list-panes",
        "-a",
        "-F",
        "#{session_name}|#{window_name}|#{pane_index}|#{pane_current_command}|#{pane_id}",
    ])?;

    if stdout.trim().is_empty() {
        println!("No active agents/panes found.");
        return Ok(());
    }

    let bold = "\x1b[1m";
    let cyan = "\x1b[36m";
    let yellow = "\x1b[33m";
    let green = "\x1b[32m";
    let reset = "\x1b[0m";

    println!("\n{bold}🌐 TMX GLOBAL DASHBOARD{reset}");
    println!(
        "===================================================================================================="
    );

    for line in stdout.lines() {
        let parts: Vec<&str> = line.split('|').collect();
        if parts.len() != 5 {
            continue;
        }
        let (session, window, pane_idx, cmd, pane_id) =
            (parts[0], parts[1], parts[2], parts[3], parts[4]);

        let cap_out = tmux_output(&["capture-pane", "-p", "-t", pane_id])?;
        let logs = cap_out.trim_end().to_string();

        println!(
            "{cyan}🖥️  [{session}] {window} (Pane {pane_idx}){reset} - {yellow}Running: {cmd}{reset}"
        );
        if logs.is_empty() {
            println!("    {green}(No output){reset}");
        } else {
            let lines: Vec<&str> = logs.lines().collect();
            let tail_start = lines.len().saturating_sub(3);
            for log_line in &lines[tail_start..] {
                let log_trim = if log_line.len() > 90 {
                    format!("{}...", &log_line[..87])
                } else {
                    (*log_line).to_string()
                };
                println!("    │ {log_trim}");
            }
        }
        println!(
            "----------------------------------------------------------------------------------------------------"
        );
    }
    Ok(())
}

fn run_monitor() -> Result<(), String> {
    let _guard = CursorGuard;
    let _ = write!(io::stdout(), "\x1B[?25l");
    loop {
        let _ = write!(io::stdout(), "\x1B[2J\x1B[1;1H");
        run_ps()?;
        println!("\n(Press Ctrl+C to exit)");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

fn print_cheatsheet(prefix: &str) {
    let cyan = "\x1b[36m";
    let yellow = "\x1b[33m";
    let reset = "\x1b[0m";
    let bold = "\x1b[1m";

    println!("\n{bold}🚀 TMX 10x AGENTIC CHEATSHEET 🚀{reset}");
    println!("=====================================================================");
    println!("{yellow}💡 WHAT IS A PREFIX?{reset}");
    println!("In tmux, you must press your 'Prefix' key before most commands.");
    println!("Think of it like a clutch: Press and release the Prefix, THEN press the action key.");
    println!("Your Prefix Key is: {bold}{cyan}{prefix}{reset}");
    println!("=====================================================================");

    println!("\n{yellow}🪟 PANES (Splitting your screen){reset}");
    println!(
        "  {cyan}{prefix} + |{reset}         : Split the current pane Vertically (Left/Right)"
    );
    println!(
        "  {cyan}{prefix} + -{reset}         : Split the current pane Horizontally (Top/Bottom)"
    );
    println!("  {cyan}{prefix} + x{reset}         : Kill/Close the current pane");
    println!(
        "  {cyan}{prefix} + z{reset}         : Zoom the current pane to full screen (Press again to unzoom)"
    );

    println!("\n{yellow}🧭 NAVIGATION (Moving around){reset}");
    println!("  {cyan}Option + Arrows{reset}   : Instantly switch panes (No Prefix needed!)");
    println!(
        "  {cyan}{prefix} + Arrows{reset}   : Switch panes (Hold Prefix & tap arrows to move repeatedly!)"
    );

    println!("\n{yellow}📑 WINDOWS & SESSIONS{reset}");
    println!("  {cyan}{prefix} + c{reset}         : Create a new window (tab)");
    println!(
        "  {cyan}{prefix} + <number>{reset}  : Switch to a specific window (e.g. {prefix} + 1)"
    );
    println!(
        "  {cyan}{prefix} + w{reset}         : ⚡ POPUP QUICK SWITCHER (Jump between projects instantly!)"
    );

    println!("\n{yellow}⚡ 10x AGENTIC POWER FEATURES{reset}");
    println!("  {cyan}{prefix} + y{reset}         : 📋 Yank last 200 lines to clipboard");
    println!(
        "  {cyan}{prefix} + P{reset}         : 🖨️  Toggle Auto-Logger (streams pane output to file)"
    );
    println!("  {cyan}{prefix} + L{reset}         : 🔒 Lock Pane (disable keyboard input)");
    println!("  {cyan}{prefix} + U{reset}         : 🔓 Unlock Pane");
    println!(
        "  {cyan}{prefix} + S{reset}         : 💾 1-CLICK CRASH REPORT (Save pane logs to tmx_crash.log)"
    );
    println!(
        "  {cyan}{prefix} + g{reset}         : 🪟 Floating Scratchpad (Great for quick curl tests!)"
    );
    println!(
        "  {cyan}{prefix} + s{reset}         : 🤖 Synchronize typing across ALL panes (Toggle on/off)"
    );
    println!("  {cyan}{prefix} + /{reset}         : 🔍 Search through your logs backwards");
    println!(
        "  {cyan}{prefix} + d{reset}         : Detach from session (Leaves it running in the background)"
    );

    if !clipboard_available() {
        println!("\n{yellow}📋 CLIPBOARD NOTE{reset}");
        println!("  Install pbcopy (macOS), wl-copy (Wayland), or xclip (X11) for yank to work.");
    }

    println!("\n{yellow}🖱️  MOUSE MODE IS ENABLED!{reset}");
    println!(
        "  You can click to switch panes, click tabs to switch windows, and drag borders to resize."
    );
    println!("=====================================================================\n");
}

fn run_config(mut cfg: AppConfig, config_dir: &Path) -> Result<AppConfig, String> {
    if !is_interactive() {
        return Err("configuration requires an interactive terminal".to_string());
    }

    println!("\n⚙️ TMX CONFIGURATION");

    cfg.prefix = read_prefix_key()?;

    cfg.editor_cmd = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Default editor command (e.g., 'cursor .')")
        .default(cfg.editor_cmd.clone())
        .allow_empty(true)
        .interact()
        .map_err(|e| format!("editor prompt failed: {e}"))?;

    cfg.env_cmd = Input::with_theme(&ColorfulTheme::default())
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
    cfg.workspace_root = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Workspace root directory")
        .default(default_workspace)
        .allow_empty(true)
        .interact()
        .map_err(|e| format!("workspace prompt failed: {e}"))?;

    save_config(&cfg, config_dir)?;
    render_tmx_conf(&cfg)?;

    println!("✅ Configuration updated successfully!\n");
    Ok(cfg)
}

fn interactive_menu(config_path: &str, cfg: AppConfig, config_dir: &Path) -> Result<(), String> {
    if !is_interactive() {
        return Err(
            "interactive menu requires a terminal — use a subcommand instead (e.g. `tmx ps`)"
                .to_string(),
        );
    }

    let options = [
        "Create Agent Workspace",
        "Switch / Attach to Session",
        "Global Dashboard (ps)",
        "Live Monitor",
        "Change Configuration",
        "Kill Session",
        "Cheatsheet",
        "Exit",
    ];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("What would you like to do?")
        .default(0)
        .items(options)
        .interact()
        .map_err(|e| format!("menu selection failed: {e}"))?;

    match selection {
        0 => run_agent_workspace(config_path, &cfg, None, false),
        1 => run_switch(config_path, None),
        2 => run_ps(),
        3 => run_monitor(),
        4 => {
            run_config(cfg, config_dir)?;
            Ok(())
        }
        5 => run_kill(None),
        6 => {
            print_cheatsheet(&cfg.prefix);
            Ok(())
        }
        _ => {
            println!("Goodbye!");
            Ok(())
        }
    }
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
        let cli = Cli::parse();

        match cli.command {
            Some(Commands::Agent { name, no_attach }) => {
                run_agent_workspace(&config_path, &cfg, name, no_attach)
            }
            Some(Commands::Attach { session }) => run_attach(&config_path, session),
            Some(Commands::Switch { session }) => run_switch(&config_path, session),
            Some(Commands::Ps) => run_ps(),
            Some(Commands::Monitor) => run_monitor(),
            Some(Commands::Kill { session }) => run_kill(session),
            Some(Commands::Config) => {
                run_config(cfg, &config_dir)?;
                Ok(())
            }
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
