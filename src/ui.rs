use std::io::{self, Write};
use std::thread;
use std::time::Duration;

// Catppuccin Mocha hex — shared with tmx.conf (reference constants for cohesion)
#[allow(dead_code)]
pub const TMUX_GREEN: &str = "#a6e3a1";
#[allow(dead_code)]
pub const TMUX_YELLOW: &str = "#f9e2af";
#[allow(dead_code)]
pub const TMUX_BLUE: &str = "#89b4fa";
#[allow(dead_code)]
pub const TMUX_PEACH: &str = "#fab387";
#[allow(dead_code)]
pub const TMUX_RED: &str = "#f38ba8";
#[allow(dead_code)]
pub const TMUX_MAUVE: &str = "#cba6f7";
#[allow(dead_code)]
pub const TMUX_TEAL: &str = "#94e2d5";
#[allow(dead_code)]
pub const TMUX_BASE: &str = "#1e1e2e";
#[allow(dead_code)]
pub const TMUX_SURFACE: &str = "#313244";
#[allow(dead_code)]
pub const TMUX_OVERLAY: &str = "#45475a";

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const DIM: &str = "\x1b[2m";

pub const ROSE: &str = "\x1b[38;5;211m";
pub const FLAMINGO: &str = "\x1b[38;5;205m";
pub const PINK: &str = "\x1b[38;5;176m";
pub const MAUVE: &str = "\x1b[38;5;135m";
pub const RED: &str = "\x1b[38;5;203m";
pub const MAROON: &str = "\x1b[38;5;181m";
pub const PEACH: &str = "\x1b[38;5;215m";
pub const YELLOW: &str = "\x1b[38;5;221m";
pub const GREEN: &str = "\x1b[38;5;114m";
pub const TEAL: &str = "\x1b[38;5;79m";
pub const SKY: &str = "\x1b[38;5;117m";
pub const SAPPHIRE: &str = "\x1b[38;5;75m";
// Extra palette tokens reserved for future UI polish.
#[allow(dead_code)]
pub const BLUE: &str = SAPPHIRE;
#[allow(dead_code)]
pub const LAVENDER: &str = "\x1b[38;5;183m";
pub const TEXT: &str = "\x1b[38;5;252m";
pub const SUBTEXT: &str = "\x1b[38;5;245m";
pub const OVERLAY: &str = "\x1b[38;5;243m";
pub const SURFACE: &str = "\x1b[38;5;240m";
pub const BASE: &str = "\x1b[38;5;237m";

pub const TMX_LOGO: &[&str] = &[
    "████████╗███╗   ███╗██╗  ██╗",
    "╚══██╔══╝████╗ ████║╚██╗██╔╝",
    "   ██║   ██╔████╔██║ ╚███╔╝ ",
    "   ██║   ██║╚██╔╝██║ ██╔██╗ ",
    "   ██║   ██║ ╚═╝ ██║██╔╝ ██╗",
    "   ╚═╝   ╚═╝     ╚═╝╚═╝  ╚═╝",
];

pub const ROBOT_MASCOT: &[&str] = &[
    "      ┌─────────────┐      ",
    "      │  ◉       ◉  │      ",
    "      │      ▽      │      ",
    "      │   ╭─────╮   │      ",
    "      └───┤ 🤖 ├───┘      ",
    "          ╰──┬──╯          ",
    "         ┌───┴───┐         ",
    "         │ AGENT │         ",
    "         └───────┘         ",
];

pub const ROCKET_LAUNCH: &[&str] = &[
    "            ▲              ",
    "           ╱ ╲             ",
    "          ╱   ╲            ",
    "         ╱  🚀 ╲           ",
    "        ╱───────╲          ",
    "       │  TMX   │          ",
    "       │  10x   │          ",
    "        ╲───────╱          ",
    "         ╲  │  ╱           ",
    "          ╲ │ ╱            ",
    "           ╲│╱             ",
    "      ～～～～～～～          ",
];

pub const DASHBOARD_FRAME: &[&str] = &[
    "  ╔══════════════════════════════════════╗",
    "  ║   🌐  GLOBAL AGENT DASHBOARD  🌐    ║",
    "  ╚══════════════════════════════════════╝",
];

pub const EMPTY_AGENTS: &[&str] = &[
    "     ┌─────────────────────────┐",
    "     │   💤  No agents yet   │",
    "     │                       │",
    "     │  Run `tmx agent foo`  │",
    "     │  to spawn your first! │",
    "     └─────────────────────────┘",
];

pub const GOODBYE_WAVE: &[&str] = &[
    "   ╭──────────────────────╮",
    "   │  👋  See you later!  │",
    "   │  Happy agent-ing! 🤖 │",
    "   ╰──────────────────────╯",
];

pub const LAYOUT_DEV: &[&str] = &[
    "  ┌────────────┬──────────┐",
    "  │            │  Server  │",
    "  │   Editor   ├──────────┤",
    "  │            │  Shell   │",
    "  └────────────┴──────────┘",
];

pub const LAYOUT_SWARM: &[&str] = &[
    "  ┌──────────┬──────────┐",
    "  │ Agent 1  │ Agent 2  │",
    "  ├──────────┼──────────┤",
    "  │ Agent 3  │ Agent 4  │",
    "  └──────────┴──────────┘",
];

pub const LAYOUT_OBSERVER: &[&str] = &[
    "  ┌────────────────┬──────┐",
    "  │                │ htop │",
    "  │     Shell      │      │",
    "  │                │      │",
    "  └────────────────┴──────┘",
];

pub const CHEAT_HEADER: &[&str] = &[
    "  ╔════════════════════════════════════════╗",
    "  ║  ⌨️   TMX 10x AGENTIC CHEATSHEET  ⚡  ║",
    "  ╚════════════════════════════════════════╝",
];

pub const KILL_SKULL: &[&str] = &[
    "      ┌───────────┐",
    "      │  ☠️  RIP  │",
    "      │  session  │",
    "      └───────────┘",
];

pub const SUCCESS_CHECK: &[&str] = &[
    "   ╭─────────────────────╮",
    "   │  ✅  All systems go │",
    "   ╰─────────────────────╯",
];

pub const SWITCH_BANNER: &[&str] = &[
    "  ╔══════════════════════════════╗",
    "  ║  ⚡  JUMP TO WORKSPACE  ⚡   ║",
    "  ╚══════════════════════════════╝",
];

/// Dev layout pane splashes.
pub const DEV_EDITOR_ART: &[&str] = &[
    "     ╭──────────────╮",
    "     │  { }  < >    │",
    "     │   EDITOR     │",
    "     ╰──────────────╯",
    "    ready to ship ✦",
];

pub const DEV_SERVER_ART: &[&str] = &[
    "        ⚡ ⚡ ⚡",
    "     ╭──────────────╮",
    "     │   SERVER     │",
    "     │  ▶▶▶ LIVE    │",
    "     ╰──────────────╯",
];

pub const DEV_SHELL_ART: &[&str] = &[
    "     ╭──────────────╮",
    "     │  $ _         │",
    "     │   SHELL      │",
    "     ╰──────────────╯",
    "    type away ✦",
];

pub const OBSERVER_SHELL_ART: &[&str] = &[
    "   ╭────────────────────╮",
    "   │  👁️   OBSERVER     │",
    "   │  watching agents   │",
    "   ╰────────────────────╯",
];

pub const OBSERVER_MONITOR_ART: &[&str] = &[
    "   ╭────────────────────╮",
    "   │  📊  MONITOR       │",
    "   │  system pulse      │",
    "   ╰────────────────────╯",
];

pub struct PaneSplash {
    pub bg: &'static str,
    pub accent: &'static str,
    pub title: &'static str,
    pub art: &'static [&'static str],
    pub footer: &'static str,
}

pub const DEV_PANES: [PaneSplash; 3] = [
    PaneSplash {
        bg: "#1e1e2e",
        accent: "183",
        title: "Editor",
        art: DEV_EDITOR_ART,
        footer: "ready to ship",
    },
    PaneSplash {
        bg: "#181825",
        accent: "215",
        title: "Server",
        art: DEV_SERVER_ART,
        footer: "standing by",
    },
    PaneSplash {
        bg: "#11111b",
        accent: "117",
        title: "Shell",
        art: DEV_SHELL_ART,
        footer: "type away",
    },
];

pub const OBSERVER_SHELL: PaneSplash = PaneSplash {
    bg: "#1e1e2e",
    accent: "135",
    title: "Observer",
    art: OBSERVER_SHELL_ART,
    footer: "watching agents",
};

pub const OBSERVER_MONITOR: PaneSplash = PaneSplash {
    bg: "#0f172a",
    accent: "79",
    title: "Monitor",
    art: OBSERVER_MONITOR_ART,
    footer: "system pulse",
};

/// Swarm grid — one unique mascot per pane.
pub const SWARM_ALPHA: &[&str] = &[
    "       ╭──────────╮       ",
    "       │ ◉      ◉ │       ",
    "       │    ▽     │       ",
    "     ╭─┴──────────┴─╮     ",
    "     │   🤖  ALPHA   │     ",
    "     ╰───────┬───────╯     ",
    "         ╭───┴───╮         ",
    "         │ READY │         ",
    "         ╰───────╯         ",
];

pub const SWARM_BRAVO: &[&str] = &[
    "          ⚡⚡⚡           ",
    "         ╱     ╲          ",
    "        │  ⚡ B  │         ",
    "        │ BRAVO │         ",
    "         ╲     ╱          ",
    "      ～～～～～～～～       ",
    "     ╭─────────────╮      ",
    "     │  ONLINE ⚡  │      ",
    "     ╰─────────────╯      ",
];

pub const SWARM_CHARLIE: &[&str] = &[
    "           ◆              ",
    "          ╱ ╲             ",
    "         ◆   ◆            ",
    "        ╱  C  ╲           ",
    "       ◆ CHARLIE ◆        ",
    "        ╲     ╱           ",
    "         ◆   ◆            ",
    "     ╭─────────────╮      ",
    "     │  ◆ STAGED ◆ │      ",
    "     ╰─────────────╯      ",
];

pub const SWARM_DELTA: &[&str] = &[
    "            ✦             ",
    "         ·  │  ·          ",
    "       ·    │    ·        ",
    "     ✦   DELTA   ✦        ",
    "       ·    │    ·        ",
    "         ·  │  ·          ",
    "            ✦             ",
    "     ╭─────────────╮      ",
    "     │  ✦ ORBIT ✦  │      ",
    "     ╰─────────────╯      ",
];

pub struct SwarmPaneStyle {
    pub bg: &'static str,
    pub accent: &'static str,
    pub codename: &'static str,
    pub art: &'static [&'static str],
}

pub const SWARM_PANES: [SwarmPaneStyle; 4] = [
    SwarmPaneStyle {
        bg: "#2d1b4e",
        accent: "135",
        codename: "ALPHA",
        art: SWARM_ALPHA,
    },
    SwarmPaneStyle {
        bg: "#1b3d32",
        accent: "114",
        codename: "BRAVO",
        art: SWARM_BRAVO,
    },
    SwarmPaneStyle {
        bg: "#3d2618",
        accent: "215",
        codename: "CHARLIE",
        art: SWARM_CHARLIE,
    },
    SwarmPaneStyle {
        bg: "#18243d",
        accent: "117",
        codename: "DELTA",
        art: SWARM_DELTA,
    },
];

/// Rotating palette for tmux window tabs (cycles for window 5, 6, …).
pub const TAB_COLORS: &[&str] = &[
    "#2d1b4e", "#1b3d32", "#3d2618", "#18243d", "#3d1b4e", "#1b2d3d",
];

pub fn hex_to_rgb(hex: &str) -> (u8, u8, u8) {
    let h = hex.trim_start_matches('#');
    if h.len() != 6 {
        return (30, 30, 46);
    }
    let parse = |s: &str| u8::from_str_radix(s, 16).unwrap_or(0);
    (parse(&h[0..2]), parse(&h[2..4]), parse(&h[4..6]))
}

/// Paint pane background via ANSI truecolor (works in macOS Terminal where tmux -P bg= does not).
pub fn ansi_bg_fill_shell(hex: &str) -> String {
    let (r, g, b) = hex_to_rgb(hex);
    format!("printf '\\033[48;2;{r};{g};{b}m\\033[2J\\033[H'")
}

pub fn window_color_script_body(paint_panes: bool) -> String {
    let mut script = String::from(
        "idx=$(tmux display -p '#{window_index}' 2>/dev/null) || exit 0\n\
         case $((idx % ",
    );
    script.push_str(&TAB_COLORS.len().to_string());
    script.push_str(")) in\n");
    for (i, color) in TAB_COLORS.iter().enumerate() {
        script.push_str(&format!("  {i}) c={color};;\n"));
    }
    script.push_str(
        "esac\n\
         wid=$(tmux display -p '#{window_id}')\n\
         swarm=$(tmux show-options -wv -t \"$wid\" @swarm_panes 2>/dev/null || echo false)\n\
         swarm=$(printf '%s' \"$swarm\" | tr -d '[:space:]')\n\
         tmux set-window-option -t \"$wid\" @tab_color \"$c\" 2>/dev/null\n\
         tmux set-window-option -t \"$wid\" window-style \"bg=$c\" 2>/dev/null\n",
    );
    if paint_panes {
        script.push_str(
            "if [ \"$swarm\" = \"true\" ]; then\n\
               exit 0\n\
             fi\n\
             r=$((16#${c:1:2})); g=$((16#${c:3:2})); b=$((16#${c:5:2}))\n\
             tmux list-panes -t \"$wid\" -F '#{pane_id}' 2>/dev/null | while IFS= read -r pid; do\n\
               [ -n \"$pid\" ] || continue\n\
               tmux select-pane -t \"$pid\" -P \"bg=$c,border=fg=$c\" 2>/dev/null\n\
               script=\"${TMPDIR:-/tmp}/tmx-fill-${pid}.sh\"\n\
               printf '%s\\n' '#!/bin/sh' \"printf '\\\\033[48;2;${r};${g};${b}m\\\\033[2J\\\\033[H'\" 'rm -f \\\"$0\\\"' > \\\"$script\\\"\n\
               chmod +x \"$script\"\n\
               tmux send-keys -t \"$pid\" \"sh '$script'\" C-m\n\
             done\n",
        );
    }
    script
}

/// Multi-line shell script: paint bg, optional fastfetch/neofetch, then colored ASCII splash.
pub fn swarm_pane_splash_script(index: usize) -> String {
    let style = &SWARM_PANES[index.min(3)];
    let mut lines = vec![ansi_bg_fill_shell(style.bg)];
    lines.push(
        "command -v fastfetch>/dev/null 2>&1&&fastfetch -l small 2>/dev/null|head -14".to_string(),
    );
    lines.push(
        "command -v neofetch>/dev/null 2>&1&&neofetch --ascii_distro auto 2>/dev/null|head -14"
            .to_string(),
    );
    lines.push(format!("printf '\\n\\033[1;38;5;{}m\\n'", style.accent));
    for line in style.art {
        let escaped = line.replace('\'', "'\\''");
        lines.push(format!("printf '%s\\n' '{escaped}'"));
    }
    let (r, g, b) = hex_to_rgb(style.bg);
    lines.push(format!(
        "printf '\\033[48;2;{r};{g};{b}m\\033[38;5;252m\\n\\n  🤖 AGENT {} — standing by\\n\\n'",
        style.codename
    ));
    lines.join("\n")
}

pub fn swarm_pane_bg(index: usize) -> &'static str {
    SWARM_PANES[index.min(3)].bg
}

/// Build a shell one-liner: paint bg, colored ASCII splash, footer line.
pub fn pane_splash_command(splash: &PaneSplash) -> String {
    let mut cmd = ansi_bg_fill_shell(splash.bg);
    cmd.push_str(&format!("printf '\\n\\033[1;38;5;{}m\\n';", splash.accent));
    for line in splash.art {
        let escaped = line.replace('\'', "'\\''");
        cmd.push_str(&format!("printf '%s\\n' '{escaped}';"));
    }
    let (r, g, b) = hex_to_rgb(splash.bg);
    cmd.push_str(&format!(
        "printf '\\033[48;2;{r};{g};{b}m\\033[38;5;252m\\n\\n  ✦ {} — {}\\n\\n';",
        splash.title, splash.footer
    ));
    cmd
}

pub fn dev_pane_splash(index: usize) -> &'static PaneSplash {
    &DEV_PANES[index.min(2)]
}

fn colorize_lines(lines: &[&str], colors: &[&str]) -> Vec<String> {
    lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            let color = colors[i % colors.len()];
            format!("{color}{line}{RESET}")
        })
        .collect()
}

pub fn print_colored_art(lines: &[&str], colors: &[&str]) {
    for line in colorize_lines(lines, colors) {
        println!("{line}");
    }
}

pub fn print_logo() {
    let colors = [MAUVE, PINK, FLAMINGO, PEACH, YELLOW, GREEN];
    print_colored_art(TMX_LOGO, &colors);
    println!("{DIM}{SUBTEXT}    10x Agentic Tmux Workspace {RESET}\n");
}

pub fn print_welcome_banner() {
    print_logo();
    print_colored_art(ROBOT_MASCOT, &[SKY, TEAL, GREEN, YELLOW, PEACH]);
    println!();
}

pub fn print_setup_banner() {
    print_colored_art(ROCKET_LAUNCH, &[PEACH, YELLOW, GREEN, TEAL, SKY, MAUVE]);
    println!(
        "\n{BOLD}{YELLOW}🚀 Welcome to TMX 10x Setup!{RESET} {DIM}Let's wire up your agent command center.{RESET}\n"
    );
}

pub fn print_dashboard_header() {
    print_colored_art(DASHBOARD_FRAME, &[SKY, SAPPHIRE, MAUVE]);
    divider(42);
}

pub fn print_empty_agents() {
    print_colored_art(EMPTY_AGENTS, &[SUBTEXT, YELLOW, GREEN, TEAL]);
}

pub fn print_cheatsheet_header() {
    print_colored_art(CHEAT_HEADER, &[PEACH, YELLOW, MAUVE, SKY]);
}

pub fn print_goodbye() {
    print_colored_art(GOODBYE_WAVE, &[ROSE, PEACH, YELLOW, GREEN]);
}

pub fn print_layout_preview(idx: usize) {
    let (art, label, color) = match idx {
        0 => (LAYOUT_DEV, "Dev Mode", GREEN),
        1 => (LAYOUT_SWARM, "Swarm Mode", MAUVE),
        _ => (LAYOUT_OBSERVER, "Observer Mode", SKY),
    };
    println!("\n{BOLD}{color}  ▶ {label}{RESET}");
    print_colored_art(art, &[color, TEAL, SAPPHIRE, SUBTEXT]);
    println!();
}

pub fn divider(width: usize) {
    println!("{SURFACE}{}", "═".repeat(width));
}

pub fn thin_divider(width: usize) {
    println!("{BASE}{}", "─".repeat(width));
}

pub fn heading(text: &str) {
    println!("\n{BOLD}{YELLOW}▸ {text}{RESET}");
}

pub fn section(title: &str) {
    println!("\n{BOLD}{PEACH}┌─ {title} {RESET}");
}

pub fn section_end() {
    println!("{PEACH}└{RESET}");
}

pub fn success(msg: &str) {
    println!("{GREEN}✅ {msg}{RESET}");
}

pub fn warn(msg: &str) {
    println!("{YELLOW}⚠️  {msg}{RESET}");
}

pub fn info(msg: &str) {
    println!("{SKY}ℹ️  {msg}{RESET}");
}

pub fn highlight(msg: &str) {
    println!("{BOLD}{MAUVE}{msg}{RESET}");
}

pub fn error(msg: &str) {
    let _ = writeln!(io::stderr(), "{RED}{BOLD}✖ error:{RESET} {RED}{msg}{RESET}");
}

pub fn die(msg: &str) -> ! {
    error(msg);
    std::process::exit(1);
}

pub fn key_combo(prefix: &str, key: &str) -> String {
    format!("{BOLD}{SKY}{prefix}{RESET} + {BOLD}{TEAL}{key}{RESET}")
}

pub fn cheat_row(prefix: &str, key: &str, desc: &str) {
    println!(
        "  {}  {DIM}→{RESET}  {TEXT}{desc}{RESET}",
        key_combo(prefix, key)
    );
}

pub fn cheat_section(title: &str) {
    println!("\n{BOLD}{PEACH}╭─ {title}{RESET}");
}

/// Live status of an agent pane, inferred from its output in `tmx monitor`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum AgentState {
    Working,
    Idle,
    NeedsInput,
    Done,
}

impl AgentState {
    /// Colored badge shown on a pane card.
    pub fn badge(&self) -> String {
        match self {
            AgentState::Working => format!("{GREEN}● working{RESET}"),
            AgentState::Idle => format!("{SUBTEXT}○ idle{RESET}"),
            AgentState::NeedsInput => format!("{YELLOW}◆ needs input{RESET}"),
            AgentState::Done => format!("{SKY}✓ done{RESET}"),
        }
    }
}

pub fn pane_card(
    session: &str,
    window: &str,
    pane_idx: &str,
    title: &str,
    cmd: &str,
    status: Option<AgentState>,
) {
    let label = if title.is_empty() {
        cmd.to_string()
    } else if cmd.is_empty() || cmd == title {
        title.to_string()
    } else {
        format!("{title} · {cmd}")
    };
    let badge = match status {
        Some(s) => format!("   {}", s.badge()),
        None => String::new(),
    };
    println!(
        "  {BOLD}{SKY}╭─ 🖥️  [{session}]{RESET} {MAUVE}{window}{RESET} {DIM}(pane {pane_idx}){RESET}{badge}"
    );
    println!("  {SKY}│{RESET}  {YELLOW}▶{RESET} {BOLD}{GREEN}{label}{RESET}");
}

pub fn pane_log_line_styled(line: &str, changed: bool) {
    if changed {
        println!("  {SKY}│{RESET}  {DIM}│{RESET} {TEXT}{line}{RESET}");
    } else {
        println!("  {SKY}│{RESET}  {DIM}│{RESET} {SUBTEXT}{line}{RESET}");
    }
}

pub fn pane_card_end() {
    println!("  {SKY}╰{OVERLAY}{}", "─".repeat(68));
}

pub fn session_created(name: &str) {
    print_colored_art(SUCCESS_CHECK, &[GREEN, TEAL]);
    success(&format!("Session '{name}' created and ready!"));
    highlight(&format!("Attach anytime: tmx attach -s {name}"));
}

pub fn session_killed(name: &str) {
    print_colored_art(KILL_SKULL, &[RED, MAROON, SUBTEXT]);
    println!("{DIM}Session '{name}' has been terminated.{RESET}");
}

pub fn monitor_footer(frame: usize) {
    let spinner = SPINNER_FRAMES[frame % SPINNER_FRAMES.len()];
    println!(
        "\n{DIM}{SUBTEXT}  {spinner} Refreshing every 2s  ·  {RESET}{YELLOW}Ctrl+C{RESET}{DIM}{SUBTEXT} to exit{RESET}"
    );
}

pub fn print_dashboard_stats(session_count: usize, pane_count: usize) {
    println!(
        "  {DIM}{SUBTEXT}{session_count} session{} · {pane_count} pane{}{RESET}",
        if session_count == 1 { "" } else { "s" },
        if pane_count == 1 { "" } else { "s" }
    );
}

pub fn clear_screen() {
    let _ = write!(io::stdout(), "\x1B[2J\x1B[1;1H");
}

pub fn pause_menu() {
    println!("\n{DIM}{SUBTEXT}Press Enter to return to the command center...{RESET}");
    let mut buf = String::new();
    let _ = io::stdin().read_line(&mut buf);
}

pub fn spawn_ritual(name: &str) {
    let launch_msg = format!("Launching '{name}'...");
    let steps: [&str; 3] = [
        "Calibrating workspace...",
        "Wiring agent grid...",
        &launch_msg,
    ];
    for step in steps {
        for _ in 0..3 {
            for &frame in SPINNER_FRAMES {
                print!("\r{GREEN}{frame}{RESET} {SKY}{step}{RESET}   ");
                let _ = io::stdout().flush();
                thread::sleep(Duration::from_millis(70));
            }
        }
        println!("\r{GREEN}✓{RESET} {SKY}{step}{RESET}   ");
    }
    println!();
}

pub fn print_switch_banner() {
    print_colored_art(SWITCH_BANNER, &[MAUVE, PEACH, YELLOW]);
    println!("{BOLD}{TEXT}  Jump to workspace{RESET} {DIM}(↑↓ and Enter){RESET}\n");
}

pub fn config_banner() {
    println!("\n{BOLD}{MAUVE}⚙️  TMX CONFIGURATION{RESET}");
    thin_divider(40);
}

pub fn menu_banner() {
    print_welcome_banner();
    println!("{BOLD}{TEXT}What would you like to do?{RESET} {DIM}(use ↑↓ and Enter){RESET}\n");
}

pub fn attach_existing(name: &str) {
    info(&format!("Session '{name}' already exists — attaching..."));
}

pub fn no_sessions() {
    print_empty_agents();
}

pub const SWARM_BANNER: &[&str] = &[
    "  ╔══════════════════════════════════════╗",
    "  ║   🐝  PARALLEL AGENT SWARM  🐝      ║",
    "  ╚══════════════════════════════════════╝",
];

pub const REVIEW_BANNER: &[&str] = &[
    "  ╔══════════════════════════════════════╗",
    "  ║   🔍  REVIEW & KEEP THE BEST  ✨     ║",
    "  ╚══════════════════════════════════════╝",
];

pub const DOCTOR_BANNER: &[&str] = &[
    "  ╔══════════════════════════════════════╗",
    "  ║   🩺  TMX DOCTOR — HEALTH CHECK      ║",
    "  ╚══════════════════════════════════════╝",
];

pub fn print_swarm_banner() {
    print_colored_art(SWARM_BANNER, &[YELLOW, PEACH, MAUVE]);
    println!(
        "{DIM}{SUBTEXT}  Each agent works in its own isolated copy — nobody clobbers anyone.{RESET}\n"
    );
}

pub fn print_review_banner() {
    print_colored_art(REVIEW_BANNER, &[MAUVE, SKY, GREEN]);
    println!();
}

pub fn print_doctor_banner() {
    print_colored_art(DOCTOR_BANNER, &[SKY, SAPPHIRE, MAUVE]);
    println!();
}

/// A single health-check line for `tmx doctor`.
pub fn doctor_check(ok: bool, label: &str, detail: &str) {
    let mark = if ok {
        format!("{GREEN}✔{RESET}")
    } else {
        format!("{RED}✖{RESET}")
    };
    if detail.is_empty() {
        println!("  {mark} {TEXT}{label}{RESET}");
    } else {
        println!("  {mark} {TEXT}{label}{RESET} {DIM}{SUBTEXT}— {detail}{RESET}");
    }
}

pub fn doctor_hint(msg: &str) {
    println!("      {YELLOW}↳ {msg}{RESET}");
}

/// Card summarizing one agent's changes in `tmx review`.
pub fn review_agent_card(
    codename: &str,
    bg: &str,
    files: usize,
    ins: usize,
    del: usize,
    untracked: usize,
) {
    let _ = bg;
    let changed = if files == 0 && untracked == 0 {
        format!("{DIM}{SUBTEXT}no changes yet{RESET}")
    } else {
        let mut parts = vec![format!(
            "{BOLD}{files}{RESET} file{}",
            if files == 1 { "" } else { "s" }
        )];
        parts.push(format!("{GREEN}+{ins}{RESET}"));
        parts.push(format!("{RED}-{del}{RESET}"));
        if untracked > 0 {
            parts.push(format!("{YELLOW}{untracked} new{RESET}"));
        }
        parts.join("  ")
    };
    println!("  {BOLD}{MAUVE}🤖 {codename}{RESET}  {DIM}│{RESET}  {changed}");
}
