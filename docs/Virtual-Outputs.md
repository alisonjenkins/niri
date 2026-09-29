# Virtual Outputs (Headless Displays)

Niri supports creating virtual headless outputs that can be used for VNC, screen sharing, or testing purposes. Virtual outputs work similarly to Sway's `create_output` command.

## Use Cases

- Remote desktop access via VNC (e.g., with wayvnc)
- Screen recording/streaming of a separate output
- Testing and development
- Running niri without physical displays (CI, servers)

## Creating Virtual Outputs

### On a Regular Session (TTY with Physical Display)

When running niri on a TTY with your physical monitor, you can create additional virtual outputs:

```bash
# Create a 1920x1080 virtual output
niri msg create-virtual-output --width 1920 --height 1080
# Output: Created virtual output: HEADLESS-1

# Create another output with different resolution
niri msg create-virtual-output --width 1280 --height 720
# Output: Created virtual output: HEADLESS-2

# Create a 120Hz output
niri msg create-virtual-output --width 1920 --height 1080 --refresh-rate 120
# Output: Created virtual output: HEADLESS-3

# List all outputs (physical + virtual)
niri msg outputs
```

### Pure Headless Mode (No Physical Display)

For servers, VMs, or remote-only access:

```bash
# Start niri in headless mode
NIRI_BACKEND=headless niri

# A default 1920x1080 HEADLESS-1 output is created automatically
# Create additional outputs if needed
niri msg create-virtual-output --width 1280 --height 720
```

## Removing Virtual Outputs

```bash
niri msg remove-virtual-output HEADLESS-1
# Output: Removed virtual output: HEADLESS-1
```

## Configuring Virtual Outputs

Virtual outputs can be configured like regular outputs:

```bash
# Set scale
niri msg output HEADLESS-1 scale 1.5

# Set transform (rotation)
niri msg output HEADLESS-1 transform 90

# Turn off
niri msg output HEADLESS-1 off

# Turn on
niri msg output HEADLESS-1 on
```

You can also configure them in your `config.kdl`:

```kdl
output "HEADLESS-1" {
    scale 1.0
    position x=1920 y=0
}
```

## Using with VNC (wayvnc)

[wayvnc](https://github.com/any1/wayvnc) is a VNC server for wlroots-based Wayland compositors.

### Setup with Physical Display + VNC

```bash
# 1. Start niri normally on your TTY
niri

# 2. Create a virtual output for VNC
niri msg create-virtual-output --width 1920 --height 1080

# 3. Start wayvnc on the virtual output
wayvnc --output HEADLESS-1

# 4. Connect from a VNC client to your machine's IP
```

### Setup for Pure Headless (Remote Only)

```bash
# 1. Start niri in headless mode (e.g., over SSH)
NIRI_BACKEND=headless niri &

# 2. Start wayvnc
WAYLAND_DISPLAY=wayland-1 wayvnc

# 3. Connect from a VNC client
```

### Headless with systemd

For a persistent headless niri session:

```ini
# ~/.config/systemd/user/niri-headless.service
[Unit]
Description=Niri Headless Session

[Service]
Type=simple
Environment=NIRI_BACKEND=headless
ExecStart=/usr/bin/niri
Restart=on-failure

[Install]
WantedBy=default.target
```

```bash
systemctl --user enable --now niri-headless
```

## CLI Reference

### create-virtual-output

```
niri msg create-virtual-output [OPTIONS]

Options:
  --width <WIDTH>              Width in pixels [default: 1920]
  --height <HEIGHT>            Height in pixels [default: 1080]
  --refresh-rate <REFRESH_RATE>  Refresh rate in Hz [default: 60]
```


Options:
- `--name <NAME>`: name the output instead of auto-generating `HEADLESS-N`.

A stable name matters to clients that remember a capture source by name. Auto-generated names are sequential, so an output removed and recreated comes back under a different name and the remembered selection silently stops resolving. Creating a named output does not consume a number, so it does not shift later generated names.

```bash
niri msg create-virtual-output --width 1280 --height 800 --name steam
# Output: Created virtual output: steam
```
### remove-virtual-output

```
niri msg remove-virtual-output <NAME>

Arguments:
  <NAME>  Name of the output to remove (e.g., "HEADLESS-1")
```

## Declaring a virtual output in the config

An `output` section with `virtual-output` is created at startup instead of waiting for a connector that will never arrive:

```kdl
output "steam" {
    virtual-output
    mode "1280x800@90"
    off
}
```

The size and refresh rate come from `mode`, defaulting to `1920x1080@60` when it is absent, since a virtual output has no connector advertising modes of its own.

A declared output survives a compositor restart and can be left `off` until something wants it. Turning it off takes it out of the layout but keeps it listed, so nothing has to recreate it — and its `mode` can be changed while it exists:

```bash
# Serve a handheld, then a television, without replacing the output.
niri msg output steam mode 1280x800@90
niri msg output steam on
niri msg output steam mode 3840x2160@60
```

Removing the `virtual-output` section removes the output on the next config reload. Outputs created over IPC are left alone by config reloads, since they belong to whoever asked for them.

## Overview columns

Every virtual output that is on gets its own column in the overview, to the right of the physical monitor's own workspaces, labelled with the virtual output's name. That column is the virtual output's real overview: the same workspaces and windows, at the same zoom.

Dragging works in both directions and between any two outputs, physical or virtual: drag a window from a column onto one of the physical monitor's workspaces, or the other way round. Dropping into the gap between two workspaces in any column creates a workspace there, exactly as it does for a physical monitor. Clicking a workspace in a virtual output's column closes the overview and enters view mode on that output, with that workspace active. Clicking one of the physical monitor's own workspaces behaves as it always has.

If the columns and the monitor's own workspaces do not all fit, the columns shrink to fit. The monitor's own workspaces keep their normal size and never move. A virtual output that is off has no column, and turning one on or off while the overview is open adds or removes its column without closing the overview.

## View mode

View mode puts one virtual output's live content on a physical monitor.

```bash
niri msg view-output steam
# Viewing steam on DP-2

niri msg view-output
# Stopped viewing steam on DP-2

niri msg --json view-output
```

Running `view-output` with no name while nothing is being viewed changes nothing and prints `Not viewing any output`.

Bind it in `config.kdl`:

```kdl
binds {
    Mod+V { view-output "steam"; }
    Mod+Shift+V { view-output; }
}
```

While viewing, the virtual output's content fills the physical monitor as far as its aspect ratio allows, with the rest of the screen as plain black bars. Overlay-layer surfaces, such as notifications, stay drawn on top of the projected content, but the monitor's own top-layer bar is hidden while viewing, the same way a fullscreen window hides it. A "Viewing: `<name>`" label shows on the monitor for 2 seconds.

Pointer input on the monitor reaches the virtual output at the matching position: clicks, scrolling and focus changes go through to whatever is under that point. A click in the bars reaches nothing. Entering view mode makes the virtual output the active monitor, so window-management commands act on it instead of the physical one.

Opening the overview while viewing shows the normal overview, with the physical monitor's own workspaces and every virtual output's column, including the one being viewed. Closing the overview returns to view mode.

View mode ends on its own, with a one-line notice on the monitor, if the virtual output being viewed turns off or is removed, or if the physical monitor showing it goes away. Running `niri msg view-output` with no name also ends it and returns the monitor to its own workspaces.

### Errors

`view-output` fails without changing anything, naming the output and the reason:

| Situation | Message |
|---|---|
| Name does not exist | `virtual output "<name>" not found` |
| Name is a physical output | `output "<name>" is not a virtual output` |
| Virtual output is off | `virtual output "<name>" is off` |
| No physical monitor to show it on | `no physical output to view "<name>" on` |

## Interaction with streams

Showing a virtual output in the overview or in view mode never changes what the virtual output itself renders, and never interrupts a screencast or capture of it. The projection is a second render of the same content, layered on top of the physical monitor's own picture; the virtual output's own render path is untouched.

The desk user's cursor stays on the physical monitor and is not drawn on the virtual output or into its stream. niri draws the pointer on a virtual output only while the pointer is physically over it, which is where a streaming client's own input puts it, so that client still sees its cursor.

## Security and limitations

Nothing is projected while the session is locked. A locked session would otherwise let its pointer and pixels reach another output through the projection; projections come back after unlocking, and a `view-output` in progress at lock time picks up again once the session unlocks.

Hot corners always act on the physical monitor the pointer is really over, never on whatever output a projection maps that position to.

Known limitations:

- A remote client and the desk user can move the pointer on a virtual output at the same time; nothing arbitrates between them.
- Content scaled to fit a physical monitor, or shrunk to fit an overview column, can look softer than at its native resolution.
- Dragging inside a shrunk overview column measures the drag distance in the physical monitor's pixels, not the virtual output's own, so the drag moves slightly more or less than it visually looks like it should. Where you drop still lands correctly, since the drop target comes from the position, not the distance dragged.

## Limitations

- Virtual outputs are not supported when running niri nested in another Wayland compositor (Winit backend)
- Virtual outputs created over IPC are not persisted across niri restarts — declare them in the config if you need them to come back
