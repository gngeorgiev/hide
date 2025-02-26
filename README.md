# H(elix)ide

Hide is a [zellij](https://github.com/zellij-org/zellij) plugin and a CLI providing something of an IDE environment for [helix](https://github.com/helix-editor/helix).

The main goal is to provide good environment-agnostic experience through a combination of the plugin and cli, with no shell scripts.

**HIDE** keeps track of all zellij panes and can focus them, write to them through the plugin regardless of your layout. Or send other commands through the cli.

> [!WARNING]
> This is a very very early stage project. It works on my machine but unless you have all of my specific settings it probably won't on yours. Even the default templates are using fish shell, which not everybody has installed. I advice to observe this from afar only. Also, the name's probably not sticking.

https://github.com/user-attachments/assets/5daa0d0b-06de-4b35-b5f9-4072a31f160f

# A nonexaustive list of configs

## Yazi

yazi.toml
```toml
[opener]
edit = [
	{ run = 'hide-cli pipe edit_file path=$@', block = true },
]
```

keymap.toml
```toml
[manager]
prepend_keymap = [
  { on = "<Enter>", run = "plugin smart-enter" },
  { on = "l", run = "plugin smart-enter" },
  { on = "o", run = "plugin smart-enter" },
  { on = "k", run = "plugin arrow -1" },
  { on = "j", run = "plugin arrow 1" },
  { on = "h", run = "plugin max-parent" },
  { on = "f", run = "plugin smart-filter" }
```

All of these plugins are official yazi plugins except for `max-parent`. `max-parent` won't allow yazi to cd to a directory above the directory it was opened with:

main.lua
```lua
--- @sync entry

return {
  entry = function(_, job)
    local root = os.getenv("SESSION_CWD") # Set by the pane's shell
    if root == nil or tostring(cx.active.current.cwd) ~= root then
      ya.manager_emit("cd", { cx.active.parent.cwd })
    end
  end,
}
```

## Zellij

With the following keybinds you'd be able to focus any pane based on its name. Don't forget to load the plugin

config.kdl
```kdl
keybinds {
  shared {
        bind "Alt Enter" {
            MessagePlugin "hide" {
                payload "0focus_pane;type=editor;"
            }
        }
        bind "Alt t" {
            MessagePlugin "hide" {
                payload "0focus_pane;type=terminal;"
            }
        }
        bind "Alt e" {
            MessagePlugin "hide" {
                payload "0focus_pane;type=file_explorer;"
            }
        }
        bind "Alt g" {
            MessagePlugin "hide" {
                payload "0focus_pane;type=lazygit;"
            }
        }
  }
}

plugins {
  hide location="/path/to/hide.wasm"
}
```

## Helix

No specific helix configs for now, although you can focus or write to panes from within helix as well:

```toml
[keys.normal.space.W]
e = ":sh hide-cli pipe focus_pane type=file_explorer"
t = ":sh hide-cli pipe focus_pane type=terminal"
g = ":sh hide-cli pipe focus_pane type=lazygit"
i = ":sh hide-cli pipe write_to_pane type=terminal;data=<esc>echo hi<enter>"
```

Some changes will have to be contributed to helix or some PRs cherry-picked for the best experience. TBD.

## Building

Build the plugin with:

```shell
cargo build --target=wasm32-wasip1
```

Reload it with:

```shell
zellij action start-or-reload-plugin hide
```
