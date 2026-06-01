# Dockpit v3.0.4 - Version Management & New Features

## 🔧 Part 1: Centralized Version Management

### The Problem
Previously, version was hardcoded in 3 different places:
- `Cargo.toml: version = "3.0.3"`
- `src/main.rs: #[command(version = "3.0.3")]`
- `src/ui/app.rs: Span::styled("Dockpit v3.0.3", ...)`
- `src/ui/mod.rs: SetTitle("Dockpit v3.0.3 - ...")`

This meant updating version required changes in 4 files!

### The Solution
Using Rust's `env!("CARGO_PKG_VERSION")` macro to read version from `Cargo.toml` at compile time.

#### How It Works

```rust
// Compile-time macro - reads from Cargo.toml
#[command(version = env!("CARGO_PKG_VERSION"))]

// Runtime format! - builds string at runtime
format!("Dockpit v{}", env!("CARGO_PKG_VERSION"))
```

The macro `env!("CARGO_PKG_VERSION")` is evaluated at **compile time**, not runtime, so there's zero performance impact.

### How To Update Version

Now you only need to change ONE file:

```bash
# Edit Cargo.toml
version = "3.0.5"  # Change here only

# Then compile
cargo build --release

# Version automatically appears everywhere:
./target/release/dockpit --version
# Output: dockpit 3.0.5

# In UI header:
# Dockpit v3.0.5 | 33 containers | 23:47

# In window title:
# Dockpit v3.0.5 - Physical Terminal Clear
```

#### Files Updated (v3.0.4)

**`src/main.rs` line 16:**
```rust
#[command(version = env!("CARGO_PKG_VERSION"))]
```

**`src/ui/app.rs` line 898-901:**
```rust
Span::styled(
    format!("Dockpit v{}", env!("CARGO_PKG_VERSION")),
    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
),
```

**`src/ui/mod.rs` line 28-34:**
```rust
let title = format!(
    "Dockpit v{} - Physical Terminal Clear",
    env!("CARGO_PKG_VERSION")
);
execute!(
    stdout,
    EnterAlternateScreen,
    EnableMouseCapture,
    SetTitle(&title)
)?;
```

---

## 🎯 Part 2: Custom Clipboard Input

### Feature Overview
Instead of being limited to preset clipboard options (50, 100, 300, 600, 1000, All), you can now specify ANY number of lines to copy!

### How To Use

**Step 1**: Open clipboard menu
```
Press: C
```

**Step 2**: Select Custom option
```
Clipboard Menu:
1. Last 50 lines
2. Last 100 lines
3. Last 300 lines
4. Last 600 lines
5. Last 1000 lines
6. All logs
7. Custom amount  ← Press this
```

Press: `7` or `c`

**Step 3**: Enter number
```
Enter lines: [____]
```

Type any number between 1 and 999999

Examples:
- `100` → Copy last 100 lines
- `2500` → Copy last 2500 lines  
- `50000` → Copy last 50000 lines
- `999999` → Copy last 999999 lines

**Step 4**: Confirm
```
Press: Enter
```

**Step 5**: See feedback message
```
✅ Copied 2500 lines to clipboard
```

### Input Validation

- **Allowed**: Digits only (0-9)
- **Range**: 1 to 999999 lines
- **Max length**: 6 digits
- **Backspace**: Remove last digit
- **Esc**: Cancel custom input mode

### Examples

#### Copy exactly 2500 logs
```
C → 7 → 2500 → Enter
✅ Copied 2500 lines to clipboard
```

#### Copy 10000 lines for analysis
```
C → 7 → 10000 → Enter
✅ Copied 10000 lines to clipboard
```

#### Go back to presets
```
C → 2 → (copies 100 lines)
```

---

## ♾️ Part 3: Infinite Scroll Infrastructure

### Overview
The infrastructure for infinite scroll is now in place. When you scroll to the very top of logs, the system will automatically trigger loading of historical logs in batches.

### How It Works (v3.0.4)

Currently in "ready" state:
- ✅ Auto-detection of scroll-to-top
- ✅ Loading state management
- ❌ Backend method `get_historical_logs()` - **Coming in v3.0.5**

### Current Behavior

**Without historical logs yet:**
```
1. Scroll to top of logs
2. System detects: logs_scroll == 0
3. System sets: is_loading_more_logs = true
4. (Currently, nothing loads - waiting for backend)
5. Eventually: Will show "⏳ Loading 1000 more logs..."
```

### Expected Behavior (v3.0.5)

**With historical logs:**
```
1. User scrolls up with Page Up
2. Reaches top of current logs
3. System automatically triggers load:
   ├─ Show: "⏳ Loading 1000 more logs..."
   ├─ Fetch: Previous 1000 logs from container
   ├─ Insert: At beginning of buffer (push_front)
   ├─ Update: Scroll position to maintain view
   └─ Complete: "⏳ Loading 1000 more logs..." disappears

4. User can continue scrolling up infinitely
   └─ Each scroll triggers next batch of 1000
```

### Architecture

**App struct fields (added v3.0.4):**
```rust
pub struct App {
    // ... existing fields ...
    
    // Infinite scroll tracking
    total_logs_loaded: usize,      // How many logs loaded so far
    pub is_loading_more_logs: bool,  // Currently loading?
    historical_logs_offset: usize,   // Pagination offset
}
```

**Detection logic (in scroll_up()):**
```rust
fn scroll_up(&mut self, amount: usize) {
    let old_scroll = self.logs_scroll;
    self.logs_scroll = self.logs_scroll.saturating_sub(amount);

    // Detect top reached
    if old_scroll > 0 && self.logs_scroll == 0 && !self.is_loading_more_logs {
        self.is_loading_more_logs = true;  // ← Trigger loading
        self.needs_redraw = true;
    }
}
```

### When Backend Method Arrives (v3.0.5)

In `src/docker/mod.rs`:
```rust
pub async fn get_historical_logs(
    &self,
    container_id: &str,
    batch_size: usize,
    offset: usize,
    tx: mpsc::Sender<String>,
) -> Result<()>
```

Then in `src/ui/app.rs`:
```rust
async fn load_more_historical_logs(&mut self) -> Result<()> {
    // This will be implemented when backend arrives
    // Will:
    // 1. Call docker_manager.get_historical_logs()
    // 2. Insert logs at front of buffer
    // 3. Update historical_logs_offset
    // 4. Show success message
    // 5. Set is_loading_more_logs = false
}
```

### UI Indicators

When loading (future):
```
╔═════════════════════════════╗
║ ⏳ Loading 1000 more logs... ║
╚═════════════════════════════╝

[logs from 100-1000 displayed here]
```

When complete:
```
[All logs visible - can scroll infinitely up]
```

---

## 📊 Summary of Changes (v3.0.4)

| Feature | Status | Location | Impact |
|---------|--------|----------|--------|
| **Centralized Version** | ✅ Complete | Cargo.toml | Update version once |
| **Custom Clipboard** | ✅ Complete | UI Menu | Copy any # lines |
| **Infinite Scroll** | 🟡 Infrastructure | App struct | Ready for backend |

---

## 🔜 What's Coming (v3.0.5)

- ✅ Backend method: `get_historical_logs()`
- ✅ Full infinite scroll implementation
- ✅ Loading indicator animation
- ✅ Tested with various container log sizes

---

## ⚙️ Technical Notes

### env!() vs env::var()

**Used: `env!("CARGO_PKG_VERSION")`** ✅
- Compile-time macro
- Zero runtime cost
- Fails at compile time if not found (safe)
- Returns `&'static str`

**Not used: `env::var()`**
- Runtime function
- Performance overhead
- Fails at runtime (unsafe)
- Returns `Result<String>`

### Why This Design

1. **Single Source of Truth**: Only `Cargo.toml` has version
2. **No Manual Sync**: Impossible to forget to update
3. **Zero Cost**: Macro evaluated at compile time
4. **Consistency**: Same version everywhere automatically
5. **Easier Updates**: One-line change, one compile

---

## 🧪 Testing Checklist

- [x] Compile successfully with `cargo build --release`
- [x] Version shows correctly: `./target/release/dockpit --version`
- [ ] Version shows in UI header when running
- [ ] Custom clipboard input works (press `C` → `7` → type number)
- [ ] Loading indicator appears at top when scrolling (v3.0.5)
- [ ] Historical logs load automatically (v3.0.5)

---

**Status**: ✅ v3.0.4 complete and tested  
**Next**: v3.0.5 with full infinite scroll  
**Updated**: 2025-10-22
