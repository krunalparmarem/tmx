use console::{Color, Style, style};
use dialoguer::theme::ColorfulTheme;

/// Catppuccin Mocha dialoguer theme — matches CLI palette.
pub fn tmx_theme() -> ColorfulTheme {
    let mauve = Color::Color256(135);
    let green = Color::Color256(114);
    let sky = Color::Color256(117);
    let text = Color::Color256(252);
    let subtext = Color::Color256(245);
    let peach = Color::Color256(215);
    let red = Color::Color256(203);

    ColorfulTheme {
        defaults_style: Style::new().for_stderr().fg(sky),
        prompt_style: Style::new().for_stderr().bold().fg(text),
        prompt_prefix: style("✦".to_string()).for_stderr().fg(mauve),
        prompt_suffix: style("›".to_string()).for_stderr().fg(subtext),
        success_prefix: style("✔".to_string()).for_stderr().fg(green),
        success_suffix: style("·".to_string()).for_stderr().fg(subtext),
        error_prefix: style("✖".to_string()).for_stderr().fg(red),
        error_style: Style::new().for_stderr().fg(red),
        hint_style: Style::new().for_stderr().fg(subtext),
        values_style: Style::new().for_stderr().fg(green),
        active_item_style: Style::new().for_stderr().bold().fg(peach),
        inactive_item_style: Style::new().for_stderr().fg(subtext),
        active_item_prefix: style("❯".to_string()).for_stderr().fg(mauve),
        inactive_item_prefix: style(" ".to_string()).for_stderr(),
        checked_item_prefix: style("✔".to_string()).for_stderr().fg(green),
        unchecked_item_prefix: style("⬚".to_string()).for_stderr().fg(mauve),
        picked_item_prefix: style("❯".to_string()).for_stderr().fg(green),
        unpicked_item_prefix: style(" ".to_string()).for_stderr(),
    }
}
