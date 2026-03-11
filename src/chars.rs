use std::sync::OnceLock;

/// Platform-safe display characters.
///
/// On Windows (or when `RUSIZE_ASCII=1` is set), all Unicode box-drawing
/// and emoji characters are replaced with pure ASCII equivalents so the
/// output renders correctly in `cmd.exe` and legacy PowerShell.
#[allow(dead_code)]
pub struct Chars {
    // Tree connectors
    pub branch: &'static str,
    pub last_branch: &'static str,
    pub vertical: &'static str,

    // Bar chart
    pub bar_full: &'static str,
    pub bar_empty: &'static str,

    // Banner frame
    pub tl: &'static str,
    pub tr: &'static str,
    pub bl: &'static str,
    pub br: &'static str,
    pub hz: &'static str,
    pub vt: &'static str,

    // Icons
    pub bolt: &'static str,
    pub search: &'static str,
    pub folder: &'static str,
    pub system: &'static str,
    pub warn: &'static str,
    pub info: &'static str,
    pub sigma: &'static str,
    pub check: &'static str,
    pub arrow: &'static str,
    pub cross: &'static str,
    pub chart: &'static str,
}

/// ASCII-safe character set for Windows / legacy terminals.
static ASCII_CHARS: Chars = Chars {
    branch: "|-- ",
    last_branch: "+-- ",
    vertical: "|   ",
    bar_full: "#",
    bar_empty: "-",
    tl: "+",
    tr: "+",
    bl: "+",
    br: "+",
    hz: "=",
    vt: "|",
    bolt: ">>",
    search: "[*]",
    folder: "[D]",
    system: "[S]",
    warn: "[!]",
    info: "[i]",
    sigma: "[=]",
    check: "[OK]",
    arrow: "=>",
    cross: "[X]",
    chart: "[~]",
};

/// Unicode character set for modern terminals (Linux, macOS, Windows Terminal).
static UNICODE_CHARS: Chars = Chars {
    branch: "\u{251C}\u{2500}\u{2500} ",      // ├──
    last_branch: "\u{2514}\u{2500}\u{2500} ", // └──
    vertical: "\u{2502}   ",                  // │
    bar_full: "\u{2588}",                     // █
    bar_empty: "\u{2591}",                    // ░
    tl: "\u{2554}",
    tr: "\u{2557}", // ╔ ╗
    bl: "\u{255A}",
    br: "\u{255D}", // ╚ ╝
    hz: "\u{2550}",
    vt: "\u{2551}",               // ═ ║
    bolt: "\u{26A1}",             // ⚡
    search: "\u{1F50D}",          // 🔍
    folder: "\u{1F4C2}",          // 📂
    system: "\u{1F5A5}\u{FE0F} ", // 🖥️
    warn: "\u{26A0}\u{FE0F} ",    // ⚠️
    info: "\u{2139}",             // ℹ
    sigma: "\u{03A3}",            // Σ
    check: "\u{2713}",            // ✓
    arrow: "\u{25B6}",            // ▶
    cross: "\u{2717}",            // ✗
    chart: "\u{1F4CA}",           // 📊
};

/// Returns the platform-appropriate character set.
///
/// Detection logic (evaluated once, cached):
/// 1. If `RUSIZE_ASCII=1` env var is set → ASCII
/// 2. If compiled for Windows → ASCII
/// 3. Otherwise → Unicode
pub fn get() -> &'static Chars {
    static CHOICE: OnceLock<bool> = OnceLock::new();
    let use_ascii = *CHOICE
        .get_or_init(|| cfg!(target_os = "windows") || std::env::var("RUSIZE_ASCII").is_ok());

    if use_ascii {
        &ASCII_CHARS
    } else {
        &UNICODE_CHARS
    }
}
