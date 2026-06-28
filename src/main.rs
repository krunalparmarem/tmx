use clap::{Parser, Subcommand};
use dialoguer::{theme::ColorfulTheme, Select, Input};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::fs;
use std::path::PathBuf;

const TMX_CONF: &str = include_str!("tmx.conf");

#[derive(Serialize, Deserialize, Default, Clone)]
struct AppConfig {
    prefix: String,
    editor_cmd: String,
    env_cmd: String,
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
    },
    /// Attach to an existing session
    Attach,
    /// Quick switch between active sessions
    Switch,
    /// Global Dashboard: List all agents and their latest logs
    Ps,
    /// Global Dashboard: Interactive live-updating monitor
    Monitor,
    /// Kill an existing session
    Kill,
    /// Change prefix or editor configuration
    Config,
    /// View the Tmux cheatsheet
    Cheat,
}

fn read_prefix_key() -> String {
    use crossterm::event::{read, Event, KeyCode, KeyModifiers};
    use crossterm::terminal::{enable_raw_mode, disable_raw_mode};

    println!("⌨️  Press the exact key combination you want to use as your Prefix:");
    println!("   (Note: The 'Command' key is intercepted by macOS. Please use Ctrl, Option/Alt, or a Function key.)\n");
    
    enable_raw_mode().unwrap();
    let mut tmux_key = String::new();
    
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
                KeyCode::Char('c') if event.modifiers.contains(KeyModifiers::CONTROL) && !event.modifiers.contains(KeyModifiers::ALT) => {
                    disable_raw_mode().unwrap();
                    println!("\nAborted.");
                    std::process::exit(1);
                }
                KeyCode::Char(c) => {
                    if c == ' ' {
                        if prefix_str.is_empty() {
                            disable_raw_mode().unwrap();
                            println!("\n⚠️  WARNING: Using 'Space' alone as a prefix means you will NEVER be able to type a space character in your terminal! Please choose a combination like 'Ctrl+Space'.\n");
                            enable_raw_mode().unwrap();
                            continue;
                        }
                        prefix_str.push_str("Space");
                    } else {
                        prefix_str.push(c);
                    }
                }
                KeyCode::F(n) => {
                    prefix_str.push_str(&format!("F{}", n));
                }
                KeyCode::Esc => {
                    disable_raw_mode().unwrap();
                    println!("\nAborted setup.");
                    std::process::exit(1);
                }
                _ => {
                    // Ignore lone modifiers
                    continue;
                }
            }
            
            tmux_key = prefix_str;
            break;
        }
    }
    disable_raw_mode().unwrap();
    println!("✅ Detected Prefix: {}", tmux_key);
    tmux_key
}

fn load_config() -> AppConfig {
    let home = std::env::var("HOME").expect("HOME not set");
    let config_dir = PathBuf::from(&home).join(".config").join("tmx");
    fs::create_dir_all(&config_dir).expect("Failed to create config dir");
    let config_file = config_dir.join("config.json");
    
    // Handle legacy prefix.txt to prevent breaking changes for early users
    let legacy_prefix = config_dir.join("prefix.txt");
    if legacy_prefix.exists() {
        let p = fs::read_to_string(&legacy_prefix).unwrap_or_else(|_| "C-a".to_string()).trim().to_string();
        let cfg = AppConfig { prefix: p, editor_cmd: "".to_string(), env_cmd: "".to_string() };
        fs::write(&config_file, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        fs::remove_file(legacy_prefix).ok();
        return cfg;
    }
    
    if config_file.exists() {
        let content = fs::read_to_string(&config_file).unwrap();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        println!("🚀 Welcome to TMX 10x Setup!");
        let prefix = read_prefix_key();
            
        let editor_cmd: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Default editor command (e.g., 'cursor .', 'code .', 'vim', or leave empty)")
            .default("".to_string())
            .allow_empty(true)
            .interact()
            .unwrap();
            
        let env_cmd: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Default environment activation command (e.g., 'source .venv/bin/activate', or leave empty)")
            .default("".to_string())
            .allow_empty(true)
            .interact()
            .unwrap();

        let cfg = AppConfig { prefix, editor_cmd, env_cmd };
        fs::write(&config_file, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
        cfg
    }
}

fn setup_tmx_conf(prefix: &str) -> String {
    let home = std::env::var("HOME").expect("HOME not set");
    let config_dir = PathBuf::from(&home).join(".config").join("tmx");
    let config_path = config_dir.join("tmx.conf");
    let conf_content = TMX_CONF.replace("{{PREFIX}}", prefix);
    fs::write(&config_path, conf_content).unwrap();
    config_path.to_string_lossy().to_string()
}

fn get_sessions() -> Vec<String> {
    let output = Command::new("tmux").arg("ls").output();
    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        stdout.lines()
            .map(|l| l.split(':').next().unwrap_or("").to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        vec![]
    }
}

fn send_cmd(pane: &str, cmd: &str) {
    if cmd.trim().is_empty() { return; }
    Command::new("tmux").args(["send-keys", "-t", pane, cmd, "C-m"]).status().unwrap();
}

fn run_agent_workspace(config_path: &str, cfg: &AppConfig, name_opt: Option<String>) {
    let name = match name_opt {
        Some(n) => n,
        None => Input::<String>::with_theme(&ColorfulTheme::default()).with_prompt("Enter project name").interact().unwrap()
    };
    
    let home = std::env::var("HOME").expect("HOME not set");
    let workspace_dir = PathBuf::from(&home).join("Projects").join(&name);
    if !workspace_dir.exists() {
        println!("Creating new project directory at {}...", workspace_dir.display());
        fs::create_dir_all(&workspace_dir).expect("Failed to create project directory");
    }
    
    if get_sessions().contains(&name) {
        println!("Session '{}' already exists. Attaching...", name);
        Command::new("tmux").current_dir(&workspace_dir).args(["-f", config_path, "attach", "-t", &name]).status().unwrap();
        return;
    }

    let layouts = vec!["Dev Mode (Code + Server + Shell)", "Swarm Mode (4-Pane Grid)", "Observer Mode (Logs + Monitor)"];
    let layout_idx = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select Layout Type")
        .default(0)
        .items(&layouts)
        .interact()
        .unwrap();

    println!("Starting Workspace '{}'...", name);
    Command::new("tmux").current_dir(&workspace_dir).args(["-f", config_path, "new-session", "-d", "-s", &name]).status().expect("Failed to start tmux");

    let base_pane = format!("{}:1.1", name);
    let wd = workspace_dir.to_string_lossy().to_string();

    match layout_idx {
        0 => { // Dev Mode
            Command::new("tmux").args(["split-window", "-h", "-c", &wd, "-l", "40%", "-t", &format!("{}:1", name)]).status().unwrap();
            Command::new("tmux").args(["split-window", "-v", "-c", &wd, "-t", &format!("{}:1.2", name)]).status().unwrap();
            send_cmd(&base_pane, &cfg.editor_cmd);
            send_cmd(&format!("{}:1.2", name), &cfg.env_cmd);
            send_cmd(&format!("{}:1.3", name), &cfg.env_cmd);
        },
        1 => { // Swarm Mode
            Command::new("tmux").args(["split-window", "-h", "-c", &wd, "-t", &format!("{}:1", name)]).status().unwrap();
            Command::new("tmux").args(["split-window", "-v", "-c", &wd, "-t", &format!("{}:1.1", name)]).status().unwrap();
            Command::new("tmux").args(["split-window", "-v", "-c", &wd, "-t", &format!("{}:1.3", name)]).status().unwrap();
            for i in 1..=4 {
                send_cmd(&format!("{}:1.{}", name, i), &cfg.env_cmd);
            }
        },
        2 => { // Observer Mode
            Command::new("tmux").args(["split-window", "-h", "-c", &wd, "-l", "30%", "-t", &format!("{}:1", name)]).status().unwrap();
            send_cmd(&base_pane, &cfg.env_cmd);
            // Use top if htop is missing
            send_cmd(&format!("{}:1.2", name), "htop || top");
        },
        _ => {}
    }

    Command::new("tmux").current_dir(&workspace_dir).args(["-f", config_path, "attach", "-t", &name]).spawn().unwrap().wait().unwrap();
}

fn run_switch(config_path: &str) {
    let sessions = get_sessions();
    if sessions.is_empty() {
        println!("No active tmux sessions.");
        return;
    }
    
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Switch to session")
        .default(0)
        .items(&sessions)
        .interact()
        .unwrap();
        
    let name = &sessions[selection];
    
    // If inside tmux, switch-client. If outside, attach.
    if std::env::var("TMUX").is_ok() {
        Command::new("tmux").args(["switch-client", "-t", name]).status().unwrap();
    } else {
        Command::new("tmux").args(["-f", config_path, "attach", "-t", name]).spawn().unwrap().wait().unwrap();
    }
}

fn run_attach(config_path: &str) {
    run_switch(config_path); // Effectively the same behavior now
}

fn run_kill() {
    let sessions = get_sessions();
    if sessions.is_empty() {
        println!("No active tmux sessions.");
        return;
    }
    
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select session to kill")
        .default(0)
        .items(&sessions)
        .interact()
        .unwrap();
        
    let name = &sessions[selection];
    Command::new("tmux").args(["kill-session", "-t", name]).status().unwrap();
    println!("Session '{}' killed.", name);
}

fn run_ps() {
    let output = Command::new("tmux")
        .args(["list-panes", "-a", "-F", "#{session_name}|#{window_name}|#{pane_index}|#{pane_current_command}|#{pane_id}"])
        .output();
        
    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        if stdout.trim().is_empty() {
            println!("No active agents/panes found.");
            return;
        }
        
        let bold = "\x1b[1m";
        let cyan = "\x1b[36m";
        let yellow = "\x1b[33m";
        let green = "\x1b[32m";
        let reset = "\x1b[0m";

        println!("\n{bold}🌐 TMX GLOBAL DASHBOARD{reset}");
        println!("====================================================================================================");
        
        for line in stdout.lines() {
            let parts: Vec<&str> = line.split('|').collect();
            if parts.len() != 5 { continue; }
            let (session, window, pane_idx, cmd, pane_id) = (parts[0], parts[1], parts[2], parts[3], parts[4]);
            
            // Fetch full pane text
            let cap_out = Command::new("tmux")
                .args(["capture-pane", "-p", "-t", pane_id])
                .output()
                .expect("Failed to capture");
            
            let mut logs = String::from_utf8_lossy(&cap_out.stdout).to_string();
            logs = logs.trim_end().to_string();
            
            println!("{cyan}🖥️  [{}] {} (Pane {}){reset} - {yellow}Running: {}{reset}", session, window, pane_idx, cmd);
            if logs.is_empty() {
                println!("    {green}(No output){reset}");
            } else {
                let lines: Vec<&str> = logs.lines().collect();
                let tail_start = if lines.len() > 3 { lines.len() - 3 } else { 0 };
                for log_line in &lines[tail_start..] {
                    let log_trim = if log_line.len() > 90 { format!("{}...", &log_line[..87]) } else { log_line.to_string() };
                    println!("    │ {}", log_trim);
                }
            }
            println!("----------------------------------------------------------------------------------------------------");
        }
    }
}

fn run_monitor() {
    // Hide cursor
    print!("\x1B[?25l");
    loop {
        // Clear screen and move to top-left
        print!("\x1B[2J\x1B[1;1H");
        run_ps();
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
    println!("Your Prefix Key is: {bold}{cyan}{}{reset}", prefix);
    println!("=====================================================================");
    
    println!("\n{yellow}🪟 PANES (Splitting your screen){reset}");
    println!("  {cyan}{} + |{reset}         : Split the current pane Vertically (Left/Right)", prefix);
    println!("  {cyan}{} + -{reset}         : Split the current pane Horizontally (Top/Bottom)", prefix);
    println!("  {cyan}{} + x{reset}         : Kill/Close the current pane", prefix);
    println!("  {cyan}{} + z{reset}         : Zoom the current pane to full screen (Press again to unzoom)", prefix);

    println!("\n{yellow}🧭 NAVIGATION (Moving around){reset}");
    println!("  {cyan}Option + Arrows{reset}   : Instantly switch panes (No Prefix needed!)");
    println!("  {cyan}{} + Arrows{reset}   : Switch panes (Hold Option & tap arrows to move repeatedly!)", prefix);
    
    println!("\n{yellow}📑 WINDOWS & SESSIONS{reset}");
    println!("  {cyan}{} + c{reset}         : Create a new window (tab)", prefix);
    println!("  {cyan}{} + <number>{reset}  : Switch to a specific window (e.g. {} + 1)", prefix, prefix);
    println!("  {cyan}{} + w{reset}         : ⚡ POPUP QUICK SWITCHER (Jump between projects instantly!)", prefix);

    println!("\n{yellow}⚡ 10x AGENTIC POWER FEATURES{reset}");
    println!("  {cyan}{} + S{reset}         : 💾 1-CLICK CRASH REPORT (Save pane logs to tmx_crash.log)", prefix);
    println!("  {cyan}{} + g{reset}         : 🪟 Floating Scratchpad (Great for quick curl tests!)", prefix);
    println!("  {cyan}{} + s{reset}         : 🤖 Synchronize typing across ALL panes (Toggle on/off)", prefix);
    println!("  {cyan}{} + /{reset}         : 🔍 Search through your logs backwards", prefix);
    println!("  {cyan}{} + d{reset}         : Detach from session (Leaves it running in the background)", prefix);

    println!("\n{yellow}🖱️  MOUSE MODE IS ENABLED!{reset}");
    println!("  You can click to switch panes, click tabs to switch windows, and drag borders to resize.");
    println!("=====================================================================\n");
}

fn run_config(mut cfg: AppConfig, config_dir: &PathBuf) -> AppConfig {
    println!("\n⚙️ TMX CONFIGURATION");
    
    cfg.prefix = read_prefix_key();
        
    cfg.editor_cmd = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Default editor command (e.g., 'cursor .')")
        .default(cfg.editor_cmd.clone())
        .allow_empty(true)
        .interact()
        .unwrap();
        
    cfg.env_cmd = Input::with_theme(&ColorfulTheme::default())
        .with_prompt("Environment activation command")
        .default(cfg.env_cmd.clone())
        .allow_empty(true)
        .interact()
        .unwrap();

    let config_file = config_dir.join("config.json");
    fs::write(&config_file, serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
    
    // Regenerate tmx.conf with new prefix
    setup_tmx_conf(&cfg.prefix);
    
    println!("✅ Configuration updated successfully!\n");
    cfg
}

fn interactive_menu(config_path: &str, mut cfg: AppConfig, config_dir: &PathBuf) {
    let options = vec!["Create Agent Workspace", "Switch / Attach to Session", "Global Dashboard (ps)", "Change Configuration", "Kill Session", "Cheatsheet", "Exit"];
    let selection = Select::with_theme(&ColorfulTheme::default()).with_prompt("What would you like to do?").default(0).items(&options).interact().unwrap();
    match selection {
        0 => run_agent_workspace(config_path, &cfg, None),
        1 => run_switch(config_path),
        2 => run_ps(),
        3 => { run_config(cfg.clone(), config_dir); },
        4 => run_kill(),
        5 => print_cheatsheet(&cfg.prefix),
        _ => println!("Goodbye!"),
    }
}

fn main() {
    // Handle Ctrl+C gracefully for monitor
    ctrlc::set_handler(move || {
        // Show cursor
        print!("\x1B[?25h");
        std::process::exit(0);
    }).unwrap_or(());

    let home = std::env::var("HOME").expect("HOME not set");
    let config_dir = PathBuf::from(&home).join(".config").join("tmx");
    let mut cfg = load_config();
    let config_path = setup_tmx_conf(&cfg.prefix);
    let cli = Cli::parse();
    
    match cli.command {
        Some(Commands::Agent { name }) => run_agent_workspace(&config_path, &cfg, name),
        Some(Commands::Attach) | Some(Commands::Switch) => run_switch(&config_path),
        Some(Commands::Ps) => run_ps(),
        Some(Commands::Monitor) => run_monitor(),
        Some(Commands::Kill) => run_kill(),
        Some(Commands::Config) => { run_config(cfg, &config_dir); },
        Some(Commands::Cheat) => print_cheatsheet(&cfg.prefix),
        None => interactive_menu(&config_path, cfg, &config_dir),
    }
}
