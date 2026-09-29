# Virgo keyboard shortcuts

Virgo uses **Cmd** on macOS and **Ctrl** on Windows/Linux for primary shortcuts. Terminal shortcuts that use Ctrl keep Ctrl on every platform. These shortcuts act on the current workspace and use the same key combinations as the corresponding [VS Code defaults](https://code.visualstudio.com/docs/reference/default-keybindings) where Virgo has an implemented action.

| Action | macOS | Windows/Linux |
| --- | --- | --- |
| Quick Open files | Cmd+P | Ctrl+P |
| Command Palette | Cmd+Shift+P or F1 | Ctrl+Shift+P or F1 |
| Save active file | Cmd+S | Ctrl+S |
| New file | Cmd+N | Ctrl+N |
| Close active tab | Cmd+W | Ctrl+W |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab | Ctrl+Tab / Ctrl+Shift+Tab |
| Find in file | Cmd+F | Ctrl+F |
| Find next / previous | Cmd+G / Cmd+Shift+G | Enter / Shift+Enter in Find |
| Search workspace | Cmd+Shift+F | Ctrl+Shift+F |
| Search and replace panel | Cmd+Shift+H | Ctrl+Shift+H |
| Explorer | Cmd+Shift+E | Ctrl+Shift+E |
| Source Control view | Ctrl+Shift+G | Ctrl+Shift+G |
| Toggle sidebar | Cmd+B | Ctrl+B |
| Toggle bottom panel | Cmd+J | Ctrl+J |
| Toggle terminal | Ctrl+` | Ctrl+` |
| New terminal | Ctrl+Shift+` | Ctrl+Shift+` |
| Split editor | Cmd + backslash | Ctrl + backslash |
| Build Cargo project | Cmd+Shift+B | Ctrl+Shift+B |
| Run Cargo project | F5 | F5 |
| Zoom in / out / reset | Cmd+= / Cmd+- / Cmd+0 | Ctrl+= / Ctrl+- / Ctrl+0 |

The Command Palette and search inputs receive focus when opened by shortcut. Palette Up/Down and Enter select commands; Escape closes it. Find Enter/Shift+Enter moves between matches and Escape closes Find. A focused terminal receives normal characters, arrows, control keys and Escape; workbench shortcuts remain available.

F5 runs `cargo run` in a new terminal for workspaces with `Cargo.toml`. Run configurations for other project types, custom keybinding editing, multi-key chords, and shortcuts for unavailable language/debug actions remain future work.
