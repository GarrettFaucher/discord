## streamdeck-equibop

An [OpenDeck](https://github.com/nekename/OpenDeck) plugin for controlling **[Equibop](https://github.com/Equicord/Equibop)** (a privacy-focused Discord client bundling Equicord/Vencord) from a stream deck.

The official Discord client exposes a local RPC/IPC socket that the original OpenDeck Discord plugin drove via OAuth (client ID/secret and `rpc.*` scopes). Equibop does not expose that voice/screenshare RPC surface, so this plugin takes a different approach: it runs a **localhost WebSocket server** that a companion **[EquibopOpenDeck userplugin](https://github.com/GarrettFaucher/equibop-opendeck)** (compiled into Equibop/Equicord) connects to as a client. Commands flow from the deck to Equibop, and Equibop pushes state back to keep button states and property inspectors in sync.

Because everything runs inside the already-authenticated Equibop client, there is **no OAuth, no client ID/secret, no Discord Developer Portal app, and no `rpc.*` scopes**. The wire contract is defined in [`PROTOCOL.md`](./PROTOCOL.md); design notes and history live in [`PLAN.md`](./PLAN.md).

> [!WARNING]
> This plugin is in an **alpha** state. It requires the companion EquibopOpenDeck userplugin to be **built into** your Equibop/Equicord install — Equibop/Vencord has no runtime plugin loading and no hot reload, so the plugin must be compiled into the client and the client restarted. See [the userplugin repo](https://github.com/GarrettFaucher/equibop-opendeck) for details.

## Architecture

This repo is the **server** half of a two-part localhost bridge. It binds `127.0.0.1:<port>` (default `6789`) and waits for the Equibop userplugin to connect as a WebSocket client.

```
┌──────────────┐        ┌─────────────────────────────────┐        ┌─────────────────────────────────┐
│  OpenDeck app │◄──────►│  streamdeck-equibop             │◄──────►│  EquibopOpenDeck userplugin     │
│  (stream deck) │  OpenAction │  (this repo — Rust plugin) │   WS   │  (WS client, inside Equibop)    │
└──────────────┘        │  WS server @ 127.0.0.1:6789     │  JSON  │  → drives Discord via Vencord   │
                        └─────────────────────────────────┘        └─────────────────────────────────┘
```

- The Rust plugin speaks OpenAction to OpenDeck and runs the WebSocket **server**.
- The userplugin connects as the WebSocket **client** (last connection wins — exactly one client is tracked) and auto-reconnects every 5 s.
- Handshake: the client sends `hello`, the server replies `ready` and primes its caches (requesting state, guilds, soundboard, and devices). Inbound feedback (state/devices/guilds/soundboard/notifications) updates button states and the open property inspectors.
- If the bridge cannot bind its port, the error is surfaced into the plugin's global settings and rendered as a red banner in the property inspector.

## Actions

All 14 actions live under the **Equibop** category (UUID prefix `com.garrettfaucher.equibop.`).

| Action | Controllers | Property Inspector |
| --- | --- | --- |
| Toggle Mute | Keypad | `pi/simple.html` |
| Toggle Deafen | Keypad | `pi/simple.html` |
| Push to Mute | Keypad | `pi/simple.html` |
| Push to Talk | Keypad | `pi/simple.html` |
| Toggle Voice Input Mode | Keypad | `pi/simple.html` |
| Toggle Video | Keypad | `pi/simple.html` |
| Toggle Screen Share | Keypad | `pi/simple.html` |
| Volume Control | Keypad, Encoder | `pi/volumecontrol.html` |
| User Volume Control | Keypad, Encoder | `pi/uservolumecontrol.html` |
| Set Audio Device | Keypad | `pi/setaudiodevice.html` |
| Text Channel | Keypad | `pi/selectchannel.html` |
| Voice Channel | Keypad | `pi/selectchannel.html` |
| Soundboard | Keypad | `pi/soundboard.html` |
| Notifications | Keypad | `pi/notifications.html` |

> Push to Mute and Push to Talk are not supported inside multi-actions.

## Requirements

- A Rust toolchain (`cargo`/`rustc`). `Cargo.toml` is `edition = "2024"`, so a recent compiler is required.
- The target triple you are building for, installed via `rustup` (e.g. `x86_64-unknown-linux-gnu`).
- [`deno`](https://deno.com/) on your `PATH` — the property inspector build runs through it.
- [OpenDeck](https://github.com/nekename/OpenDeck) installed.
- Equibop with the companion [EquibopOpenDeck userplugin](https://github.com/GarrettFaucher/equibop-opendeck) compiled in (see [Setup](#setup)).

## Build & install

Building and installing is driven by [`build.sh`](./build.sh), which takes **exactly three arguments, in order**:

```
./build.sh <output_directory> <binary_name> <target_triple>
```

The canonical invocation (from the script's own usage text) installs straight into the OpenDeck plugins directory:

```sh
cd /path/to/streamdeck-equibop
./build.sh ~/.config/opendeck/plugins/com.garrettfaucher.equibop.sdPlugin equibop x86_64-unknown-linux-gnu
```

`build.sh` then:

1. Clears stale property-inspector output (`rm -rf assets/pi`) so removed routes/components don't ship as orphan chunks.
2. Builds the property inspector — `cd pi && deno task build` (which runs `vite build`; output lands in `assets/pi/`).
3. Wipes and recreates the output directory, copying `assets/` (manifest, icons, actions, built PI) into it.
4. Builds the release binary — `cargo build --release`.
5. Copies the binary to `<output_directory>/<binary_name>-<target_triple>`, matching the per-target `CodePaths` in the manifest.

After this, the install directory (e.g. `~/.config/opendeck/plugins/com.garrettfaucher.equibop.sdPlugin/`) contains `manifest.json`, the icon, `actions/`, `pi/`, and the binary `equibop-<target_triple>`. Restart OpenDeck to pick up a freshly installed plugin folder. The plugin then appears under the **Equibop** category with the 14 actions above; drag one onto a key as usual.

> [!NOTE]
> If the PI build fails resolving `@openaction/svelte-pi` (stale `deno.lock`/`node_modules` pinned to an old version), run `cd pi && deno install` first to refresh the lock, then rebuild. A clean clone is unaffected.

## Setup

The plugin does nothing on its own — it needs a client to connect to its bridge.

1. **Install the companion userplugin.** Follow the [EquibopOpenDeck](https://github.com/GarrettFaucher/equibop-opendeck) README to compile it into your Equibop/Equicord build and enable it under **Settings → Plugins**. Remember there is no runtime loading or hot reload — every change to the userplugin needs a rebuild and a restart.
2. **Match the Bridge Port.** Open any Equibop action's property inspector in OpenDeck. Under the **Equibop Bridge** section there is a **Bridge Port** number input (1–65535, default **6789**) that writes `port` into the plugin's global settings; the Rust side rebinds when it changes. This must equal the port the userplugin connects to (its `BRIDGE_PORT`, default **6789**). Keep both sides on the same port.
3. **Confirm the connection.** With both halves running, the plugin logs `Equibop bridge listening on 127.0.0.1:6789` and the userplugin connects on Equibop startup (auto-reconnecting every 5 s). Buttons should now reflect live Discord state.

> [!NOTE]
> For **Toggle Screen Share / Go Live** to actually capture (rather than stream black frames), Equibop must run under Wayland ozone — launch it with `equibop --wayland` or set the flag in `~/.config/equibop-flags.conf`. Starting a share pops the OS screen-source portal picker, which is a Wayland security requirement and cannot be bypassed from a renderer plugin; stopping a share is headless.

## Troubleshooting

**Buttons do nothing / a red ✕ alert flashes / log shows `No Equibop client connected; dropping <command>`.**
No WebSocket client is attached to the bridge. Either Equibop isn't running, the EquibopOpenDeck userplugin isn't built in / enabled, or the two sides are on different ports. Start Equibop with the userplugin enabled (Settings → Plugins → EquibopOpenDeck) and make the PI's Bridge Port equal the userplugin's `BRIDGE_PORT` (both default 6789). The client reconnects every 5 s.

**PI shows a red `Error:` banner like `Could not bind the bridge on port 6789: <os error>`.**
The bridge couldn't bind `127.0.0.1:<port>` — usually the port is already in use (an old plugin process still holding it, or another app). Free the port or change the Bridge Port; a port change restarts the server. The banner clears on a successful bind.

**Screen share goes LIVE but viewers see a black screen.**
Equibop isn't running in Wayland ozone mode, so capture goes through XWayland and produces black frames. Launch Equibop with `equibop --wayland` (or the equivalent flag in `~/.config/equibop-flags.conf`).

**Rebuilt the PI but old UI persists.**
A removed route/component shipped as an orphaned Vite chunk because `assets/pi` wasn't purged before the build. `build.sh` now runs `rm -rf assets/pi` first; if you build the PI by hand, clear it yourself.

**Changes don't take effect.**
There is no hot reload. On the userplugin side, `pnpm i && pnpm build` then restart Equibop (a renderer reload / Ctrl+R reconnects the WS client). On this side, rebuild and reinstall the plugin, then restart OpenDeck.

**Second Equibop instance breaks the bridge, or only one side's port was changed.**
The server tracks exactly one client (last connection wins) and a port change must be applied to **both** sides — the PI's Bridge Port and the userplugin's compiled `BRIDGE_PORT`. Run a single Equibop instance and keep both ports equal.

### Where to look

- **Plugin (this repo) log:** `~/.local/share/opendeck/logs/plugins/com.garrettfaucher.equibop.sdPlugin.log` (general OpenDeck log: `~/.local/share/opendeck/logs/opendeck.log`). Healthy startup line: `Equibop bridge listening on 127.0.0.1:6789`. Other useful lines include `Equibop client connected from <peer>`, `Equibop client hello: <client> (protocol v1)`, and `No Equibop client connected; dropping …`.
- **Userplugin (Equibop renderer) console:** DevTools logs are prefixed `[EquibopOpenDeck]`, e.g. `[EquibopOpenDeck] connected to bridge on :6789`.

## Alpha note

This plugin and its companion userplugin are **alpha**. The bridge protocol is `v1` and must be kept byte-for-byte in sync between this repo and the userplugin (bump the version on both sides when changing it). Expect rough edges, and prefer matching commits of [streamdeck-equibop](https://github.com/GarrettFaucher/streamdeck-equibop) and [EquibopOpenDeck](https://github.com/GarrettFaucher/equibop-opendeck).
