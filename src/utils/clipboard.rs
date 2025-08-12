use anyhow::{Context, Result};
use arboard::Clipboard;

pub struct ClipboardManager {
    clipboard: Option<Clipboard>,
}

impl ClipboardManager {
    pub fn new() -> Self {
        let clipboard = match Clipboard::new() {
            Ok(cb) => Some(cb),
            Err(_) => None,
        };
        
        Self { clipboard }
    }

    pub fn copy_to_clipboard(&mut self, text: &str) -> Result<()> {
        if let Some(ref mut clipboard) = self.clipboard {
            clipboard.set_text(text)
                .context("Failed to set clipboard text")?;
            Ok(())
        } else {
            // Fallback to command-line tools if arboard doesn't work
            self.copy_with_command(text)
        }
    }

    fn copy_with_command(&self, text: &str) -> Result<()> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        // Try different clipboard commands based on the system
        let commands = [
            ("xclip", vec!["-selection", "clipboard"]),
            ("xsel", vec!["--clipboard", "--input"]),
            ("pbcopy", vec![]),  // macOS
            ("clip.exe", vec![]), // WSL
        ];

        for (cmd, args) in commands.iter() {
            if let Ok(mut process) = Command::new(cmd)
                .args(args)
                .stdin(Stdio::piped())
                .spawn()
            {
                if let Some(stdin) = process.stdin.as_mut() {
                    if stdin.write_all(text.as_bytes()).is_ok() {
                        if process.wait()?.success() {
                            return Ok(());
                        }
                    }
                }
            }
        }

        Err(anyhow::anyhow!("No clipboard tool available"))
    }

    pub fn get_from_clipboard(&mut self) -> Result<String> {
        if let Some(ref mut clipboard) = self.clipboard {
            clipboard.get_text()
                .context("Failed to get clipboard text")
        } else {
            self.get_with_command()
        }
    }

    fn get_with_command(&self) -> Result<String> {
        use std::process::Command;

        // Try different clipboard commands based on the system
        let commands = [
            ("xclip", vec!["-selection", "clipboard", "-o"]),
            ("xsel", vec!["--clipboard", "--output"]),
            ("pbpaste", vec![]),  // macOS
            ("powershell.exe", vec!["-command", "Get-Clipboard"]), // WSL
        ];

        for (cmd, args) in commands.iter() {
            if let Ok(output) = Command::new(cmd).args(args).output() {
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).to_string());
                }
            }
        }

        Err(anyhow::anyhow!("No clipboard tool available"))
    }
}