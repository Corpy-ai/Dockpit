//! OSC 52 terminal clipboard support.
//!
//! OSC 52 is an ANSI escape sequence (`ESC ] 52 ; c ; <base64> BEL`) that asks
//! the terminal emulator to set the system clipboard. The key property is that
//! the *local* terminal (where the user sits) interprets it — so it works
//! transparently over SSH, where no X11/Wayland/clipboard tool on the remote
//! host could ever reach the user's clipboard.
//!
//! Terminal support varies: kitty, alacritty, wezterm, foot, ghostty, iTerm2
//! and Konsole (with clipboard write enabled) honor it; older GNOME Terminal
//! does not. When inside tmux/screen the sequence must be wrapped in the
//! multiplexer's passthrough so it reaches the outer terminal.

/// Max base64 payload most terminals reliably accept for OSC 52. Larger
/// selections are commonly truncated or ignored, which would silently corrupt
/// the paste — better to refuse and point the user to the export option.
const MAX_BASE64_LEN: usize = 100_000;

#[derive(Debug, PartialEq, Eq)]
pub enum Osc52Error {
    /// The encoded payload is larger than terminals reliably accept.
    TooLarge { bytes: usize },
}

impl std::fmt::Display for Osc52Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Osc52Error::TooLarge { bytes } => write!(
                f,
                "selection too large for terminal clipboard ({} KB) — use Export (6) instead",
                bytes / 1024
            ),
        }
    }
}

impl std::error::Error for Osc52Error {}

/// Build the full OSC 52 escape sequence that sets the system clipboard to
/// `text`, applying tmux/screen passthrough wrapping when running inside a
/// multiplexer (auto-detected from the environment).
pub fn osc52_sequence(text: &str) -> Result<String, Osc52Error> {
    build_sequence(text, in_tmux(), in_screen())
}

/// Pure core of [`osc52_sequence`] with the multiplexer flags passed in, so it
/// can be tested independently of the runtime environment.
fn build_sequence(text: &str, tmux: bool, screen: bool) -> Result<String, Osc52Error> {
    let payload = base64_encode(text.as_bytes());
    if payload.len() > MAX_BASE64_LEN {
        return Err(Osc52Error::TooLarge { bytes: text.len() });
    }

    // OSC 52, clipboard ("c") selection, BEL-terminated.
    let inner = format!("\x1b]52;c;{payload}\x07");
    Ok(wrap(inner, tmux, screen))
}

/// Wrap the raw OSC sequence for terminal multiplexers so it reaches the outer
/// terminal. Pure (flags passed in) for testability.
fn wrap(inner: String, tmux: bool, screen: bool) -> String {
    if tmux {
        // tmux DCS passthrough: ESC P tmux ; <inner with ESC doubled> ESC \
        // Requires `allow-passthrough on` (default in tmux >= 3.3).
        let escaped = inner.replace('\x1b', "\x1b\x1b");
        return format!("\x1bPtmux;\x1b{escaped}\x1b\\");
    }
    if screen {
        // GNU screen DCS passthrough.
        return format!("\x1bP{inner}\x1b\\");
    }
    inner
}

fn in_tmux() -> bool {
    std::env::var_os("TMUX").is_some()
}

fn in_screen() -> bool {
    std::env::var("TERM")
        .map(|t| t.starts_with("screen"))
        .unwrap_or(false)
}

/// Standard base64 encoding (RFC 4648) with `=` padding. Hand-rolled to avoid a
/// dependency for this single use.
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied();
        let b2 = chunk.get(2).copied();

        let n = ((b0 as u32) << 16) | ((b1.unwrap_or(0) as u32) << 8) | (b2.unwrap_or(0) as u32);

        out.push(TABLE[((n >> 18) & 0x3f) as usize] as char);
        out.push(TABLE[((n >> 12) & 0x3f) as usize] as char);
        out.push(if b1.is_some() {
            TABLE[((n >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if b2.is_some() {
            TABLE[(n & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        // RFC 4648 §10 test vectors.
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn base64_handles_non_ascii_bytes() {
        // "ñ" is 0xC3 0xB1 in UTF-8.
        assert_eq!(base64_encode("ñ".as_bytes()), "w7E=");
    }

    #[test]
    fn sequence_has_osc52_envelope() {
        // Use the pure core so the test doesn't depend on $TMUX/$TERM of the
        // process running `cargo test`.
        let seq = build_sequence("foobar", false, false).unwrap();
        assert_eq!(seq, "\x1b]52;c;Zm9vYmFy\x07");
    }

    #[test]
    fn wrap_tmux_doubles_escapes_and_uses_dcs() {
        let inner = "\x1b]52;c;Zm9v\x07".to_string();
        let wrapped = wrap(inner, true, false);
        assert_eq!(wrapped, "\x1bPtmux;\x1b\x1b\x1b]52;c;Zm9v\x07\x1b\\");
    }

    #[test]
    fn wrap_screen_uses_dcs_passthrough() {
        let inner = "\x1b]52;c;Zm9v\x07".to_string();
        let wrapped = wrap(inner, false, true);
        assert_eq!(wrapped, "\x1bP\x1b]52;c;Zm9v\x07\x1b\\");
    }

    #[test]
    fn wrap_plain_is_unchanged() {
        let inner = "\x1b]52;c;Zm9v\x07".to_string();
        assert_eq!(wrap(inner.clone(), false, false), inner);
    }

    #[test]
    fn oversized_payload_is_rejected() {
        // 100_000 base64 chars => 75_000 raw bytes is the boundary; go past it.
        let big = "x".repeat(80_000);
        assert!(matches!(
            osc52_sequence(&big),
            Err(Osc52Error::TooLarge { .. })
        ));
    }
}
