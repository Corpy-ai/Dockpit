use anyhow::{Context, Result};
use arboard::Clipboard;

pub struct ClipboardManager {
    clipboard: Option<Clipboard>,
}

impl ClipboardManager {
    pub fn new() -> Self {
        Self {
            clipboard: Clipboard::new().ok(),
        }
    }

    pub fn copy_to_clipboard(&mut self, text: &str) -> Result<()> {
        // Prefer external CLI tools first: they hand the data to the OS
        // clipboard / fork a daemon that owns the selection independently of
        // this process. arboard's X11/Wayland selection is tied to our process
        // lifetime, so text copied through it can vanish once the TUI exits
        // (no clipboard manager → empty paste). The CLI tools persist.
        if self.copy_with_command(text).is_ok() {
            return Ok(());
        }

        // Fallback: in-process clipboard for environments without any CLI tool.
        if let Some(ref mut clipboard) = self.clipboard {
            return clipboard
                .set_text(text)
                .context("Failed to set clipboard text");
        }

        Err(anyhow::anyhow!("No clipboard mechanism available"))
    }

    fn copy_with_command(&self, text: &str) -> Result<()> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        // Ordered by platform; the first tool that exists and succeeds wins.
        let commands: &[(&str, &[&str])] = &[
            ("wl-copy", &[]),                        // Wayland
            ("xclip", &["-selection", "clipboard"]), // X11
            ("xsel", &["--clipboard", "--input"]),   // X11
            ("pbcopy", &[]),                         // macOS
            ("clip.exe", &[]),                       // WSL / Windows
        ];

        for (cmd, args) in commands {
            // Skip silently if the tool isn't installed.
            let Ok(mut process) = Command::new(cmd)
                .args(*args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            else {
                continue;
            };

            // Write the payload, then CLOSE stdin by dropping the handle so the
            // tool sees EOF and exits/daemonizes. Previously stdin was kept open
            // (`as_mut`) across `wait()`, so tools like xclip blocked forever
            // waiting for more input — freezing the whole UI.
            if let Some(mut stdin) = process.stdin.take() {
                if stdin.write_all(text.as_bytes()).is_err() {
                    let _ = process.kill();
                    let _ = process.wait();
                    continue;
                }
                // `stdin` is dropped here → pipe closes → EOF.
            }

            match process.wait() {
                Ok(status) if status.success() => return Ok(()),
                _ => continue,
            }
        }

        Err(anyhow::anyhow!(
            "No clipboard tool available (tried wl-copy, xclip, xsel, pbcopy, clip.exe)"
        ))
    }
}

impl Default for ClipboardManager {
    fn default() -> Self {
        Self::new()
    }
}
