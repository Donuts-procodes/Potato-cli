use colored::*;
use std::io::{stdout, Write};
use tokio::time::{sleep, Duration};

/// 8-Bit Arcade Marquee Banner for Potato CLI
pub const ARCADE_BANNER: &str = r#"
  ╔══════════════════════════════════════════════════════════════╗
  ║    👾 🕹️  P O T A T O   C L I  🕹️ 👾                         ║
  ║       ★ 8-BIT AUTONOMOUS SUPER LOOP AGENT ENGINE ★         ║
  ║       INSERT COIN • 1-UP READY • TYPE /help FOR CMDS       ║
  ╚══════════════════════════════════════════════════════════════╝
"#;

/// 8-Bit Arcade Frame Sets for Animated Spinners
pub const INVADER_FRAMES: &[&str] = &[
    "👾  [ ▰ ▱ ▱ ▱ ]",
    "👾  [ ▰ ▰ ▱ ▱ ]",
    "👾  [ ▰ ▰ ▰ ▱ ]",
    "👾  [ ▰ ▰ ▰ ▰ ]",
    "🕹️   [ ▱ ▰ ▰ ▰ ]",
    "🕹️   [ ▱ ▱ ▰ ▰ ]",
    "🕹️   [ ▱ ▱ ▱ ▰ ]",
    "🕹️   [ ▱ ▱ ▱ ▱ ]",
    "🪙  [ ▰ ▱ ▱ ▱ ]",
    "🪙  [ ▰ ▰ ▱ ▱ ]",
    "🪙  [ ▰ ▰ ▰ ▱ ]",
    "🪙  [ ▰ ▰ ▰ ▰ ]",
    "⚡  [ ▱ ▰ ▰ ▰ ]",
    "⚡  [ ▱ ▱ ▰ ▰ ]",
    "⚡  [ ▱ ▱ ▱ ▰ ]",
    "⚡  [ ▱ ▱ ▱ ▱ ]",
];

pub const PACMAN_FRAMES: &[&str] = &[
    "ᗧ · · · ·",
    "· ᗧ · · ·",
    "· · ᗧ · ·",
    "· · · ᗧ ·",
    "· · · · ᗧ",
    "· · · 👻 ·",
    "· · 👻 · ·",
    "· 👻 · · ·",
];

/// Plays a rapid, snappy 8-bit retro arcade boot animation.
pub async fn play_arcade_boot() {
    let boot_steps = [
        ("👾", "[ INITIALIZING 8-BIT POTATO NEURAL CORE... ]"),
        ("🕹️ ", "[ MOUNTING CARTRIDGE: SUPER_LOOP.ROM...   ]"),
        ("🪙", "[ INSERT COIN: 1-UP READY PLAYER ONE!     ]"),
    ];

    for (icon, step) in boot_steps {
        print!("\r  {} {} ", icon, step.cyan().bold());
        let _ = stdout().flush();
        sleep(Duration::from_millis(70)).await;
    }
    // Erase the line cleanly
    print!("\r\x1B[2K");
    let _ = stdout().flush();
}

/// Formats an 8-bit stage header banner for turn execution.
pub fn format_stage_header(turn: usize, max_turns: usize, score: u64) -> String {
    let top = "╔══════════════════════════════════════════════════════════════╗";
    let title = format!(
        "║  👾 STAGE {:02}/{:02} ── [SCORE: {:06}] ── [POTATO CORE]       ║",
        turn, max_turns, score
    );
    let bottom = "╚══════════════════════════════════════════════════════════════╝";

    format!(
        "\n{}\n{}\n{}",
        top.cyan().bold(),
        title.yellow().bold(),
        bottom.cyan().bold()
    )
}

/// Formats an 8-bit Stage Clear victory banner.
pub fn format_stage_clear(summary: &str) -> String {
    let top =    "╔══════════════════════════════════════════════════════════════╗";
    let line1 =  "║         ★ ★ ★   S T A G E   C L E A R !   ★ ★ ★          ║";
    let line2 =  "║  🏆 HIGH SCORE ACHIEVED • 1-UP BONUS AWARDED                 ║";
    let line3 =  "║  MISSION ACCOMPLISHED • AUTONOMOUS RUN SUCCESSFUL            ║";
    let bottom = "╚══════════════════════════════════════════════════════════════╝";

    format!(
        "\n{}\n{}\n{}\n{}\n{}\n\n{}\n",
        top.green().bold(),
        line1.yellow().bold(),
        line2.cyan().bold(),
        line3.green(),
        bottom.green().bold(),
        summary
    )
}

/// Formats an 8-bit Game Over / Rollback banner.
pub fn format_game_over(msg: &str) -> String {
    let top =    "╔══════════════════════════════════════════════════════════════╗";
    let line1 =  "║                 💀  G A M E   O V E R  💀                 ║";
    let line2 =  "║  CONTINUE?  9 ... 8 ... 7 ... [ROLLING BACK STATE]           ║";
    let bottom = "╚══════════════════════════════════════════════════════════════╝";

    format!(
        "\n{}\n{}\n{}\n{}\n{}\n",
        top.red().bold(),
        line1.yellow().bold(),
        line2.red(),
        bottom.red().bold(),
        msg.dimmed()
    )
}

/// Renders an 8-bit pixel progress bar (e.g. `[▰▰▰▰▱▱▱▱] 50%`)
pub fn render_pixel_bar(current: f64, max: f64, width: usize) -> String {
    if max <= 0.0 || width == 0 {
        return format!("[{}]", "▱".repeat(width));
    }
    let ratio = (current / max).clamp(0.0, 1.0);
    let filled = (ratio * width as f64).round() as usize;
    let unfilled = width.saturating_sub(filled);

    let bar = format!("{}{}", "▰".repeat(filled), "▱".repeat(unfilled));

    if ratio < 0.6 {
        format!("[{}]", bar.green())
    } else if ratio < 0.85 {
        format!("[{}]", bar.yellow())
    } else {
        format!("[{}]", bar.red())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_bar_rendering() {
        let bar = render_pixel_bar(5.0, 10.0, 8);
        assert!(bar.contains("▰▰▰▰▱▱▱▱"));

        let full = render_pixel_bar(10.0, 10.0, 4);
        assert!(full.contains("▰▰▰▰"));

        let empty = render_pixel_bar(0.0, 10.0, 4);
        assert!(empty.contains("▱▱▱▱"));
    }

    #[test]
    fn test_stage_header_format() {
        let header = format_stage_header(1, 20, 500);
        assert!(header.contains("STAGE 01/20"));
        assert!(header.contains("SCORE: 000500"));
    }

    #[test]
    fn test_stage_clear_format() {
        let clear = format_stage_clear("All tests passed");
        assert!(clear.contains("S T A G E   C L E A R"));
        assert!(clear.contains("All tests passed"));
    }

    #[test]
    fn test_game_over_format() {
        let over = format_game_over("Syntax error");
        assert!(over.contains("G A M E   O V E R"));
        assert!(over.contains("Syntax error"));
    }
}
