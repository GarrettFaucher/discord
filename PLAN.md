# Plan: Fork the OpenDeck Discord plugin to control Equibop (`streamdeck-equibop` + Equibop userplugin)

> **This document is the canonical, cross-session task log.** It is written to survive frequent context
> compaction and to be picked up by many independent agent sessions. Every task is self-contained.
> Once implementation starts, this file is committed to the `streamdeck-equibop` repo as `PLAN.md`
> (task **T0.2**) and becomes the source of truth there.

---

## How to use this document (READ FIRST, every session)

1. **Orient**: read the *Context*, *Architecture*, *WebSocket protocol contract*, and *Reference appendix*
   sections before touching code. They contain everything the research established so you do **not** need
   to re-investigate Equibop/OpenDeck/Vencord.
2. **Pick work**: go to the *Task log*. Choose the next task whose checkbox is `[ ]` **and whose every
   dependency is `[x]`**. Do not start a task with unmet deps.
3. **Track**: set the task to `[~]` (in progress) when you start. When done, set `[x]` and append a
   one-line `→ done: <what/commit>` note under it. If you discover new work, add a new task with a fresh
   ID rather than silently expanding an existing one.
4. **Status legend**: `[ ]` not started · `[~]` in progress · `[x]` done · `[!]` blocked (add a `→ blocked:`
   note explaining why and what would unblock it).
5. **Two repos**: every task is tagged **(R)** = `streamdeck-equibop` (the Rust OpenDeck plugin, currently
   the local checkout at `/home/garrett/git/discord`) or **(V)** = the Equibop/Vencord userplugin repo.
6. **Keep the contract sacred**: the *WebSocket protocol contract* is the shared interface between the two
   repos. If you change it, bump its version and update both sides + the contract section in the same task.
7. **Commit discipline**: small commits per task; reference the task ID in the commit message
   (e.g. `T1.5: route action handlers through WS bridge`). Update this file's checkbox in the same commit.

---

## Context (why we're doing this)

The user runs **Equibop** (a privacy-focused fork of **Vesktop**, which embeds **Equicord**, a fork of the
**Vencord** Discord client mod) instead of the official Discord desktop client, and uses **OpenDeck** (an
open-source Elgato Stream Deck alternative) to drive Discord from a stream deck.

The existing OpenDeck **Discord plugin** (`OpenActionPlugins/discord`, a Rust + Svelte plugin) controls
Discord exclusively through the **official Discord RPC IPC socket** (`discord-ipc-0`) using OAuth2 and
privileged `rpc.*` scopes. **Equibop does not implement that voice/video/screenshare RPC surface** — its
bundled arRPC only does Rich Presence (mute/deafen is an explicitly open, unimplemented arRPC feature:
OpenAsar/arrpc#79). So the official plugin simply cannot drive Equibop. That is the gap this project closes.

**Outcome**: two repos that together give a stream deck near-parity control over Equibop:
1. **`streamdeck-equibop`** — a fork of the OpenDeck Discord plugin whose Discord-comms layer is swapped
   from Discord-RPC-IPC to a **local WebSocket bridge**.
2. **An Equibop/Vencord userplugin** — runs *inside* the authenticated Equibop client, connects to the
   bridge, and executes voice/video/channel actions by calling Discord's internal webpack modules directly,
   pushing state back for live button feedback.

**Why this is actually better than the original for Equibop users**: running inside the client means we
**drop OAuth, client ID/secret, the Discord Developer Portal setup, and all whitelist-gated `rpc.*` scopes
entirely**. Feasibility per action becomes "does Discord's internal JS expose it" rather than "is the app
whitelisted for this RPC scope."

**IMPORTANT scope correction (confirmed during planning):** the local checkout is a stale **v0.2.2 with only
4 actions**. The user asked to fork **upstream latest = v0.5.0, which has 14 actions** (including the
"begin streaming" the user remembered = *Toggle Video* + *Toggle Screen Share*). The plan targets **0.5.0
parity**, built **core-first**: a 6-action MVP, then expand to the full 14 by feasibility tier.

---

## Architecture

```
   ┌─────────────────────┐         OpenDeck WS (openaction SDK)        ┌──────────────────────────┐
   │      OpenDeck app    │ ◄──────────────────────────────────────►  │  streamdeck-equibop (R)  │
   │  (button presses,    │   keyDown/keyUp/dialRotate/willAppear...   │  Rust plugin process      │
   │   key images/state)  │   setState/setImage/setFeedback...         │                          │
   └─────────────────────┘                                            │  • openaction client      │
                                                                       │  • NEW: WS *server*       │
                                                                       │    on 127.0.0.1:<port>    │
                                                                       └─────────────┬────────────┘
                                                                                     │  localhost WebSocket
                                                                                     │  (our protocol, see contract)
                                                                       ┌─────────────▼────────────┐
                                                                       │  Equibop (Electron)       │
                                                                       │  renderer + Equicord      │
                                                                       │  ┌──────────────────────┐ │
                                                                       │  │ equibop userplugin(V)│ │
                                                                       │  │ • WS *client*        │ │
                                                                       │  │ • calls Discord      │ │
                                                                       │  │   internal modules   │ │
                                                                       │  │ • Flux subs → state  │ │
                                                                       │  └──────────────────────┘ │
                                                                       └──────────────────────────┘
```

### Key decisions (settled)
- **Rust = WS server, Vencord = WS client.** The Electron renderer can be a WebSocket *client* but cannot
  host a server; the Rust process already runs an event loop and can also bind a localhost port. This
  mirrors the proven arRPC pattern (arRPC server; Vencord's WebRichPresence is the client).
- **One Rust process, two roles.** It is simultaneously a WS *client* to OpenDeck (via `openaction::run()`)
  and a WS *server* for Equibop. Spawn the server with `tokio::spawn(...)` **before** the blocking
  `run(...).await` in `main`. The runtime is already `#[tokio::main]` + `rt-multi-thread`.
- **Drop Discord RPC/OAuth entirely.** Delete `oauth.rs`, the `discord-ipc-rust` dependency, and the
  `clientId/clientSecret/accessToken` settings. The PI's only setting becomes the bridge **port** (default
  `6789`).
- **Equibop side is a userplugin now, full fork later.** Repo V ships just the plugin + install docs.
  Either way Equibop must be **built from source** (`pnpm i && pnpm build`, restart) — there is no runtime
  plugin loading. Document this friction prominently.
- **Parity semantics are exact** for the core voice actions: upstream `SetVoiceSettings{mute,deaf}` and the
  Vencord `setSelfMute/setSelfDeaf` both control *local self* mute/deafen.
- **Feedback via Flux.** The userplugin subscribes to Discord Flux events and pushes `stateUpdate` messages
  so button state stays correct even when the user mutes from Discord's own UI (replaces upstream's
  `VoiceSettingsUpdate` subscription + `GetVoiceSettings`).

### Repos & locations
- **(R) `streamdeck-equibop`** — fork of `OpenActionPlugins/discord`. Local: `/home/garrett/git/discord`
  (origin `github.com/GarrettFaucher/discord`, currently v0.2.2; must be advanced to upstream **v0.5.0**
  baseline before refactoring — see **T0.3**). Rename repo/dir to `streamdeck-equibop`.
- **(V) Equibop userplugin** — new repo (suggested `github.com/GarrettFaucher/equibop-opendeck`). Contains
  `index.tsx` (the `definePlugin`), `README.md` (install-from-source instructions), and a copy of the
  protocol contract. Built by dropping it into an Equibop/Equicord source checkout under `src/userplugins/`.

---

## WebSocket protocol contract  (v1 — the shared interface; keep both repos in sync)

- Transport: text JSON frames over `ws://127.0.0.1:<port>` (default **6789**). **R is the server**, **V is the
  client**. Every message has a `type`. Unknown `type`s must be ignored (forward-compat).
- Connection lifecycle: on connect, **V** sends `hello`; **R** replies `ready`; **V** immediately sends a
  full `stateUpdate` (and, when asked, `devices`/`guilds`/`soundboard`). On disconnect, **R** marks "no
  client" (action handlers then `show_alert`); **V** auto-reconnects with backoff.

### V → R  (events / feedback)
| type | fields | meaning |
|---|---|---|
| `hello` | `client`, `version` | client announced itself |
| `stateUpdate` | `mute`,`deaf`,`inputMode`("PUSH_TO_TALK"\|"VOICE_ACTIVITY"),`video`(bool),`screenshare`(bool),`channelId`(string\|null) | full self voice/video state for button feedback |
| `devices` | `input:[{id,name}]`,`output:[{id,name}]`,`currentInput`,`currentOutput`,`inputVolume`(0–100),`outputVolume`(0–200) | populates Volume/Set-Audio-Device PIs |
| `guilds` | `[{id,name,voice:[{id,name}],text:[{id,name}]}]` | populates Text/Voice Channel PIs |
| `soundboard` | `[{guildId,soundId,name,emojiName?}]` | populates Soundboard PI |
| `notification` | `channelId`,`title?`,`body?` | a Discord notification arrived |

### R → V  (commands)
| type | fields | maps to (Vencord) |
|---|---|---|
| `ready` | — | handshake reply |
| `requestState` / `requestDevices` / `requestGuilds` / `requestSoundboard` | — | V responds with the matching event |
| `setMute` | `value`(bool) | `setSelfMute(value)` |
| `setDeafen` | `value`(bool) | `setSelfDeaf(value)` |
| `toggleMute` / `toggleDeafen` | — | read store, call setter with negation |
| `setVoiceInputMode` | `mode` | media-engine `setMode(...)` |
| `toggleVoiceInputMode` | — | read current mode, set the other |
| `setInputVolume` / `setOutputVolume` | `value` | media-engine volume setters |
| `setUserVolume` | `userId`,`value` | `setLocalVolume(userId,value,...)` |
| `setInputDevice` / `setOutputDevice` | `deviceId` | media-engine device setters |
| `setVideo` / `toggleVideo` | `value?` | camera enable/toggle action |
| `setScreenShare` / `toggleScreenShare` | `value?` | Go Live start/stop (**hard — see risks**) |
| `selectVoiceChannel` | `channelId`(string\|null) | `selectVoiceChannel(channelId)` |
| `selectTextChannel` | `guildId`,`channelId` | navigate/transition to channel |
| `playSoundboard` | `guildId`,`soundId` | soundboard play action |

> The MVP (T-phase 1–4) only needs: `hello`,`ready`,`stateUpdate`,`requestState`,`setMute`,`setDeafen`,
> `toggleMute`,`toggleDeafen`,`selectVoiceChannel`,`selectTextChannel`,`guilds`. The rest land in later phases.

---

## Action inventory & feasibility (upstream v0.5.0 → Equibop)

All 14 upstream actions are RPC-based; the port re-implements each via Discord internals in the userplugin.
Feasibility reflects the **Vencord** path. "Verify in DevTools" = confirm the exact finder/store at impl time.

| # | Action (UUID suffix) | Upstream RPC mechanism | Vencord/Equicord path (best-known) | Feasibility | Tier |
|---|---|---|---|---|---|
| 1 | Toggle Mute (`togglemute`) | `SetVoiceSettings{mute}` | `MediaEngineStore.isSelfMute()` + `setSelfMute(bool)` | **Easy** | MVP |
| 2 | Toggle Deafen (`toggledeafen`) | `SetVoiceSettings{deaf}` | `MediaEngineStore.isSelfDeaf()` + `setSelfDeaf(bool)` | **Easy** | MVP |
| 3 | Push to Mute (`pushtomute`) | `SetVoiceSettings{mute}` on down/up | `setSelfMute(true/false)` | **Easy** | MVP |
| 4 | Push to Talk (`pushtotalk`) | `SetVoiceSettings{mute}` on down/up | `setSelfMute(false/true)` | **Easy** | MVP |
| 12 | Voice Channel (`voicechannel`) | `SELECT_VOICE_CHANNEL`/`GetSelectedVoiceChannel` + voice-state subs | `selectVoiceChannel(id\|null)`; `SelectedChannelStore.getVoiceChannelId()` | **Easy** | MVP |
| 11 | Text Channel (`textchannel`) | `SELECT_TEXT_CHANNEL` | `NavigationRouter`/channel-select action; lists from `GuildStore`/`ChannelStore` | **Easy–Med** | MVP |
| 5 | Toggle Voice Input Mode (`togglevoiceinputmode`) | `SetVoiceSettings{mode}` | media-engine `getMode()`/`setMode(PUSH_TO_TALK\|VOICE_ACTIVITY)` | **Medium** | 2 |
| 8 | Volume Control (`volumecontrol`, dial) | `SetVoiceSettings{input/output.volume}` | media-engine input/output volume getters+setters | **Medium** | 2 |
| 9 | User Volume Control (`uservolumecontrol`) | `SET_USER_VOICE_SETTINGS{user_id,volume}` | `setLocalVolume(userId,value)`; `getLocalVolume` | **Medium** | 2 |
| 10 | Set Audio Device (`setaudiodevice`) | `SetVoiceSettings{input/output.device_id}` | media-engine device list + setters | **Medium** | 2 |
| 13 | Soundboard (`soundboard`) | `PlaySoundboardSound{guild_id,sound_id}`; `GetSoundboardSounds` | soundboard play action; `SoundboardStore` for list | **Medium** | 2 |
| 6 | Toggle Video / camera (`togglevideo`) | `SET_VIDEO`-style cmd + `VideoStateUpdate` sub | camera/video toggle action (verify) | **Med–Hard** | 3 |
| 14 | Notifications (`notifications`) | `SUBSCRIBE NOTIFICATION_CREATE` → cache channel_id | observe Flux `MESSAGE_CREATE`/notification dispatch | **Med–Hard** | 3 |
| 7 | Toggle Screen Share / Go Live (`togglescreenshare`) | `START/STOP_STREAM`-style cmd + `ScreenshareStateUpdate` sub | Go Live programmatic start needs source picker; Equibop has special Linux screenshare path | **Hard/Uncertain** | 3 |

**Upstream OAuth scopes (for reference; the port needs NONE of these):** `rpc`, `rpc.voice.read`,
`rpc.voice.write`, `rpc.video.read`, `rpc.video.write`, `rpc.screenshare.read`, `rpc.screenshare.write`,
`rpc.notifications.read`, `identify`.

---

## Task log

> Phases are dependency-ordered. Within a phase, tasks can often run in parallel across sessions. Each task
> lists **Repo · Files · Do · Deps · Accept**. MVP = end of Phase 5 (6 actions working end-to-end).

### Phase 0 — Bootstrapping & shared contract
- [x] **T0.1 (R+V)** Author the protocol contract file.
  - Files: `PROTOCOL.md` (root of both repos, identical copy).
  - Do: copy the *WebSocket protocol contract* section above verbatim; mark it `v1`.
  - Deps: none. Accept: file exists in both repos; both READMEs link it.
  - → done: `PROTOCOL.md` (v1) created in streamdeck-equibop (R). The V copy lands with T0.4.
- [x] **T0.2 (R)** Commit this plan as the living tracker.
  - Files: `PLAN.md` (root of R).
  - Do: copy this document into `streamdeck-equibop/PLAN.md`; from here on, update PLAN.md (not the
    `~/.claude/plans` copy) as tasks progress.
  - Deps: T0.3 (so it lands on the 0.5.0 baseline). Accept: PLAN.md committed; checkboxes reflect reality.
  - → done: PLAN.md now lives at repo root on branch `equibop-port` atop the v0.5.0 baseline. **This file is
    the source of truth from here on** — update it, not the `~/.claude/plans` copy.
- [~] **T0.3 (R)** Rebase the fork onto upstream **v0.5.0** and rename.
  - Files: whole repo. Do: `git fetch upstream`; advance `main` to upstream **v0.5.0** (`71171b3…`) as the new
    baseline (recommended: reset/rebase the fork onto v0.5.0 — the only fork-unique commit is the 0.2.x PI
    style sync, which 0.5.0 supersedes; confirm before discarding). Rename GitHub repo + local dir
    `discord` → `streamdeck-equibop`; update `Cargo.toml` `package.name`, `build.sh` usage, README title.
  - Deps: none. Accept: `git log` shows the 14-action v0.5.0 tree; `cargo build` succeeds on the unmodified
    upstream baseline; binary/package named `streamdeck-equibop` (or `equibop`).
  - → done (baseline): `main` **fast-forwarded to v0.5.0** — lossless (the fork had **0 unique commits**, so
    nothing was discarded; the noted "confirm before discarding" risk did not apply). Confirmed 14 actions in
    the manifest. Work branch **`equibop-port`** created off the new baseline.
  - → done (in-repo identity): the **package/binary/manifest/PI/UUID rebrand is complete** as part of T7.4
    (`oadiscord`→`equibop`, `me.amankhanna.oadiscord.*`→`com.garrettfaucher.equibop.*`, Author/Name/Category).
    `cargo build --release` and `build.sh` both succeed under the new identity.
  - → PENDING (only the *names of the containers*): renaming the **GitHub repo** `discord`→`streamdeck-equibop`
    and the **local dir** `/home/garrett/git/discord`→`…/streamdeck-equibop` remain a **manual user step** — the
    GitHub rename needs the user (no `gh` CLI here) and renaming the cwd mid-session is disruptive. Neither affects
    the built plugin's identity, which is already fully `com.garrettfaucher.equibop` / `equibop`.
- [x] **T0.4 (V)** Scaffold the Equibop userplugin repo.
  - Files: `index.tsx`, `README.md`, `LICENSE`, `PROTOCOL.md`.
  - Do: minimal `definePlugin({name:"EquibopOpenDeck",description,authors})`; README documents the build-from-
    source install (clone Equibop/Equicord → copy into `src/userplugins/equibopOpendeck/` → `pnpm i && pnpm
    build` → restart) and the no-hot-reload constraint.
  - Deps: T0.1. Accept: repo builds as a no-op plugin inside an Equibop checkout and appears in the plugin list.
  - → done: new repo at **`/home/garrett/git/equibop-opendeck`** (branch `main`, commit `623485b`): no-op
    `definePlugin` scaffold (`index.tsx`) with `start()`/`stop()` stubs marked `TODO(T3.1)`, build-from-source
    `README.md` (no-hot-reload documented), **GPL-3.0-or-later** `LICENSE` (matches upstream + Vencord), and a
    copy of `PROTOCOL.md` (this satisfies the V copy noted in T0.1). The WS client itself is Phase 3.

### Phase 1 — Rust: strip Discord-RPC, stand up the WS server  (R)
- [x] **T1.1 (R)** Swap dependencies.
  - Files: `Cargo.toml`. Do: remove `discord-ipc-rust`, `reqwest`; add `tokio-tungstenite`, `futures-util`;
    keep `openaction`,`serde`,`serde_json`,`tokio`,`log`,`simplelog`. Deps: T0.3. Accept: `cargo metadata` resolves.
  - → done: removed `discord-ipc-rust` + `reqwest`; added `tokio-tungstenite = { 0.28, default-features=false,
    features=["handshake"] }` (server only, no TLS) + `futures-util`; widened `tokio` features to add
    `net`,`sync`,`time`. All deps were already in `Cargo.lock`/cache, so `cargo build --offline` resolves clean.
- [x] **T1.2 (R)** Define protocol types.
  - Files: `src/protocol.rs`. Do: serde enums for all contract messages — `ServerCommand` (`#[serde(tag="type")]`,
    camelCase) and `ClientMessage`. Deps: T0.1. Accept: a unit test round-trips each example JSON from PROTOCOL.md.
  - → done: `src/protocol.rs` with `ServerCommand`/`ClientMessage` (internal `type` tag, camelCase variants +
    per-variant `rename_all` for `userId`/`deviceId`/`channelId`/`guildId`/`soundId`/`inputMode`), shared
    `Device`/`Channel`/`Guild`/`Sound`/`Devices` structs, `#[serde(other)] Unknown` catch-all, and defaults so
    partial `stateUpdate`s parse. **7 unit tests pass** (`cargo test`) covering tags, round-trips, defaults,
    unknown-tolerance. Also clarified the wire encodings in `PROTOCOL.md` (guilds/soundboard nesting keys,
    optional `selectTextChannel.guildId`, hello-priming lifecycle).
- [x] **T1.3 (R)** WS server module.
  - Files: `src/ws_server.rs`. Do: `tokio-tungstenite` server on `127.0.0.1:<port>`; accept loop; store the
    single active client sender in a `OnceLock<RwLock<Option<...>>>` mirroring the old `discord_client()`
    pattern; `send_command(ServerCommand) -> Result` errs when no client; on inbound `ClientMessage`, route
    `stateUpdate`→feedback, cache `devices`/`guilds`/`soundboard`. Deps: T1.1,T1.2. Accept: integration test —
    dummy client connects, `hello`↔`ready`, `send_command` delivered, disconnect clears state.
  - → done: `src/ws_server.rs` — `serve(port)` accept loop; active client `mpsc::UnboundedSender` in a
    `OnceLock<RwLock<Option<…>>>`; per-connection split into a writer task (mpsc→sink) + reader loop;
    `send_command(ServerCommand) -> Result<(),()>` errs (and warns) when no client; on `hello` it replies
    `ready` then primes caches (`requestState`/`requestGuilds`/`requestSoundboard`/`requestDevices`); inbound
    messages route into `feedback`; disconnect clears the client **only if still us** (`same_channel` guard) and
    calls `feedback::on_client_disconnected()`. NOTE: the automated dummy-client loopback test was **not** added
    (would need the `connect` client feature/dep); covered instead by the bind smoke test + the MVP e2e (T5.3)
    and the adversarial review workflow.
- [x] **T1.4 (R)** Preserve state feedback, delete RPC routing.
  - Files: rename/rewrite `src/rpc_events.rs` → `src/feedback.rs`. Do: keep `apply_voice_state`/`update_action_state`/
    `visible_instances`+`set_state` logic; drive it from inbound `stateUpdate`; delete `ReturnedCommand`/
    `ReturnedEvent`/`SocketClosed`/error-4006 handling. Deps: T1.3. Accept: `apply_voice_state(mute=true,deaf=false)`
    sets ToggleMute→1, ToggleDeafen→0.
  - → done: `src/rpc_events.rs` deleted, replaced by `src/feedback.rs`. `apply_state_update(mute,deaf,inputMode,
    video,screenshare,channelId)` drives ToggleMute (`mute||deaf`), ToggleDeafen, ToggleVoiceInputMode (+stores
    the mode string), ToggleVideo, ToggleScreenshare button states and the voice-channel cache; `apply_devices`/
    `apply_guilds`/`apply_soundboard`/`apply_notification` repopulate caches + PIs; `on_client_disconnected`
    clears all client-specific state. New shared `src/state.rs` holds `current_voice_channel`. All RPC/4006/socket
    handling removed.
- [x] **T1.5 (R)** Rewrite `main.rs` wiring.
  - Files: `src/main.rs`; delete `src/oauth.rs`, `src/client.rs`. Do: replace `DiscordSettings` with
    `Settings{port:u16=6789, error:Option<String>}`; in `main`, after `register_action(...)`,
    `tokio::spawn(ws_server::serve(port))` **before** `run(...).await`; on `did_receive_global_settings` port
    change, restart the server. Deps: T1.1–T1.4. Accept: `cargo build --release`; running the binary logs
    "listening on 127.0.0.1:6789".
  - → done: `src/main.rs` rewritten; `src/oauth.rs` + `src/client.rs` deleted. `DiscordSettings` → `Settings{
    port:u16=6789, error:Option<String>}` with serde default 6789. `restart_server(port)` (aborts the prior
    serve `JoinHandle`, rebinds) is called before `run(...).await`; `did_receive_global_settings` restarts the
    server on a port change. **`cargo build --release` clean (0 warnings)**; the binary logs `Equibop bridge
    listening on 127.0.0.1:6789` (smoke-confirmed) before openaction panics on the missing `-port` host flag.

### Phase 2 — Rust: route the 6 MVP actions through the bridge  (R)
- [x] **T2.1 (R)** Port the 4 voice-settings actions.
  - Files: `src/actions/voice_settings.rs`. Do: replace `emit_command(SetVoiceSettings…)` with
    `ws_server::send_command(...)` — ToggleMute.key_up→`SetMute{!current}`; ToggleDeafen→`SetDeafen`;
    PushToMute down/up→`SetMute{true}`/`{false}`; PushToTalk down/up→`SetMute{false}`/`{true}`. Keep optimistic
    `set_state` on ok, `show_alert` on no-client. Keep UUIDs (rename in T7). Deps: T1.5. Accept: each handler
    sends the right variant (send-capture test); `cargo build`.
  - → done: `src/actions/voice_settings.rs` — `send_with_state(instance, cmd, next_state)` replaces
    `update_voice_setting`; ToggleMute/ToggleDeafen send `SetMute`/`SetDeafen{!current}`, PushToMute/PushToTalk
    send absolute `SetMute{true/false}` on down/up, ToggleVoiceInputMode sends `SetVoiceInputMode{mode}` (reads
    the cached mode string). Optimistic `set_state` on ok, `show_alert` on no-client. UUIDs unchanged.
- [x] **T2.2 (R)** Port Voice Channel + Text Channel actions.
  - Files: `src/actions/channel.rs`, the `selectchannel` PI. Do: VoiceChannel→`SelectVoiceChannel{channelId|null}`
    (toggle join/leave via cached `channelId` from `stateUpdate`); TextChannel→`SelectTextChannel{guildId,channelId}`.
    PI populates its guild/channel pickers from cached `guilds` (request via `requestGuilds`). Deps: T1.5.
    Accept: pressing the buttons emits the right commands; PI lists guilds/channels.
  - → done (Rust side): `src/actions/channel.rs` — VoiceChannel join/leave toggles via `current_voice_channel`
    and sends `SelectVoiceChannel{channelId|null}`; TextChannel sends `SelectTextChannel{guildId,channelId}`.
    The per-guild `GetChannels` round-trip + `request_channels`/`ChannelKind` machinery was **removed**: the
    `guilds` event now carries nested `voice`/`text` channel lists (`CachedGuild` extended), so the PI filters
    locally. ⚠️ **SILENT BREAKAGE until T4.1** (confirmed by the session-2 review): the `selectchannel` PI
    (`pi/src/routes/selectchannel/+page.svelte`) is still the unchanged upstream version — it does the old
    two-step `request_channels` round-trip (whose Rust `send_to_plugin` handler was **deleted**) and reads a flat
    `payload.channels` the server no longer sends, while ignoring the new nested `guild.voice`/`guild.text`. Net:
    **Voice/Text Channel selection is non-functional** (dropdown stuck on "No channels available") until the PI
    is rewritten in **T4.1**. Not the MVP *build* gate, but it IS an MVP *functionality* gate. ✅ **RESOLVED in
    T4.1 this session** — the PI now reads the nested channel lists.
  - → NOTE (early Tier-2/3 R-port): removing `discord-ipc-rust` forced **all 14** action handlers off the old
    Discord types, so the Rust side of the Tier-2/3 actions was ported now too (they send their protocol command
    via `send_command`): Volume (`setInputVolume`/`setOutputVolume`, dial), Set Audio Device (`setInput/OutputDevice`,
    validates against cached `devices`), Soundboard (`playSoundboard`), Video (`toggleVideo`), Screen Share
    (`toggleScreenShare`), Notifications (`selectTextChannel{guildId:null}`), User Volume (`setUserVolume`). Their
    **V-side handlers remain the Phase 6/7 gates.** Two protocol gaps surfaced for **T6.3**: (a) there is no
    `setUserMute` command yet (per-user Mute currently `show_alert`s with a `TODO(T6.3)`), and (b) no inbound
    message feeds `user_voice_settings_map`, so the User Volume PI user list is empty and relative user-volume
    alerts until that message is added. The perceptual `to_linear`/`to_discord` volume math was intentionally
    dropped (the client now speaks Discord's native 0–100 / 0–200 scale).

### Phase 3 — Vencord userplugin: WS client + MVP handlers  (V)
- [x] **T3.1 (V)** WS client lifecycle.
  - Files: `index.tsx`. Do: model on Vencord `src/plugins/arRPC.web/index.tsx` — connect in `start()`,
    `close()` in `stop()`, `onopen`→send `hello` then initial `stateUpdate`, `onmessage`→dispatch by `type`,
    `onclose`/`onerror`→reconnect with backoff (~5s). Deps: T0.4. Accept: with R running, R logs `hello` and
    receives the initial `stateUpdate`.
- [x] **T3.2 (V)** Voice command handlers + verify finders in DevTools.
  - Files: `index.tsx`. Do: `const Media = findByProps("setSelfMute","setSelfDeaf")` (fallback
    `findByProps("toggleSelfMute","toggleSelfDeaf")`); `MediaEngineStore` from `@webpack/common`. Implement
    `setMute`/`setDeafen`/`toggleMute`/`toggleDeafen` and `requestState`. **Verify the finder resolves in
    DevTools.** Deps: T3.1. Accept: each command toggles the real Discord mute/deafen indicator.
- [~] **T3.3 (V)** Channel command handlers.
  - Files: `index.tsx`. Do: `const VoiceActions = findByPropsLazy("selectVoiceChannel")`; implement
    `selectVoiceChannel(id|null)` and `selectTextChannel` (verify the text-nav action — `NavigationRouter`/
    channel select); implement `requestGuilds` reading `GuildStore`/`ChannelStore`/`SortedGuildStore`. Deps: T3.1.
    Accept: commands join/leave voice and jump text channels; `guilds` event populates the R-side cache.
- [~] **T3.4 (V)** State feedback via Flux.
  - Files: `index.tsx`. Do: `flux:{ AUDIO_TOGGLE_SELF_MUTE(){…}, AUDIO_TOGGLE_SELF_DEAF(){…}, VOICE_STATE_UPDATES(){…} }`
    handlers read `MediaEngineStore` + `SelectedChannelStore.getVoiceChannelId()` and push `stateUpdate`. Deps: T3.1.
    Accept: muting from Discord's own UI flips the stream-deck button state.
  - → done (T3.1/T3.2): `equibop-opendeck` `c21161a` (first draft) → `05b9974` (route finders through Equicord's
    real `@webpack/common` exports — `VoiceActions.toggleSelfMute/toggleSelfDeaf`, `ChannelActions.selectVoiceChannel`,
    `NavigationRouter`, `GuildChannelStore.getChannels`) → `96cf759` on the **R** side fixed the startup deadlock that
    was the real blocker. **User confirmed live: Toggle Mute + Toggle Deafen drive Equibop and reflect button state.**
  - → T3.3/T3.4 `[~]`: channel handlers + Flux feedback are implemented and **re-verified against Equicord source**
    (all finders re-confirmed at `src/webpack/common/{utils,stores}.ts`; flux `AUDIO_TOGGLE_SELF_MUTE`/`SELF_DEAF`/
    `VOICE_STATE_UPDATES` confirmed via `vcNarrator`) and the plugin **compiles cleanly into the Equibop bundle**
    (`pnpm build`, dist/equibop/renderer.js). Promote to `[x]` after the user live-tests voice/text channel switching
    and Discord-UI→button feedback (part of T5.3).

### Phase 4 — Rust PI rework  (R)
- [x] **T4.1 (R)** Replace the Discord-app PI with a bridge-port PI **and migrate the channel PI**.
  - Files: `pi/src/lib/ApplicationSettings.svelte`, `pi/src/routes/selectchannel/+page.svelte` (and other
    `pi/src/routes/...` as needed). Do: (1) remove clientId/secret/OAuth instructions; add a "Bridge Port" number
    bound to `$globalSettings.port` (default 6789), an error banner from `$globalSettings.error` (now also set by
    the Rust side on a bridge **bind failure** — see `set_bridge_error`), and a short "install the Equibop plugin"
    block linking repo V. (2) **Fix the channel PI silent breakage (see T2.2):** rewrite `selectchannel/+page.svelte`
    to read `guild.voice` / `guild.text` from the `guilds` payload (Voice page → voice list, Text page → text list)
    and **delete** the `request_channels` round-trip + `payload.channels` handling (the Rust `send_to_plugin`
    handler no longer exists). Deps: T1.5.
    Accept: `vite build` succeeds; PI shows only the port field + status; the Voice/Text Channel dropdowns
    populate from the nested guild data and a selection drives the bridge.
  - → done: `ApplicationSettings.svelte` rewritten — clientId/secret/accessToken/OAuth removed; a **Bridge Port**
    number bound to `$globalSettings.port` (default 6789), the `$globalSettings.error` banner (now also fed by the
    Rust bind-failure path), and an EquibopOpenDeck install block. `selectchannel/+page.svelte` rewritten to read
    nested `guild.voice`/`guild.text` chosen by `$actionInfo.action` (voice vs text), defaulting guild+channel and
    **dropping `request_channels`/`payload.channels` entirely**. `deno task build` succeeds and `svelte-check`
    passes (0 errors); output regenerated to `assets/pi/` (gitignored). ⚙️ env note: the local PI build first
    needed `deno install` — the gitignored `deno.lock`/`node_modules` were stale at `@openaction/svelte-pi@1.0.1`;
    a fresh install resolves the `^1.1.0` constraint to 1.1.0 (which exports `actionInfo`/`eventTarget`). A clean
    clone is unaffected.
- [x] **T4.2 (R)** Rebrand PI package.
  - Files: `pi/package.json`. Do: rename `oadiscord-pi`→`equibop-pi`; rebuild; confirm `assets/pi/*` regenerate.
    Deps: T4.1. Accept: PI builds; generated HTML references the new bundle.
  - → done with the coordinated rename (T7.4): `pi/package.json` name `oadiscord-pi`→`equibop-pi`; PI rebuilt via
    `build.sh` and reinstalled. (Vite output bundles are content-hash-named so the package `name` never appears in the
    generated HTML — purely cosmetic, but now consistent with the rest of the identity.)

### Phase 5 — MVP build, package & end-to-end verification
- [x] **T5.1 (R)** Build/package the Rust plugin.
  - Files: `build.sh`, `.github/workflows/build.yml`. Do: binary base name → `equibop`; ensure manifest
    `CodePaths` match; `./build.sh <out> equibop <triple>` installs to `~/.config/opendeck/plugins/`. Deps: T2.*,T4.*.
    Accept: bundle builds; CI green.
  - → done (build/install path verified): `./build.sh ~/.config/opendeck/plugins/me.amankhanna.oadiscord.sdPlugin
    oadiscord x86_64-unknown-linux-gnu` builds the PI + release binary and installs cleanly; the installed binary
    logs `Equibop bridge listening on 127.0.0.1:6789` and the installed PI shows the new **Bridge Port** UI (no
    stale Client ID/Secret). **Fixed a packaging bug:** `build.sh` did not purge `assets/pi` before building, so a
    removed PI route/component shipped as an **orphan chunk** (the old Client-Secret UI was still present in the
    first install); added `rm -rf assets/pi` before the PI build. ⚠️ env note: run `cd pi && deno install` first
    if the local `deno.lock` is stale (see T4.1). **Rename now done (T7.4):** Cargo package + binary base →
    `equibop`, installed via `./build.sh ~/.config/opendeck/plugins/com.garrettfaucher.equibop.sdPlugin equibop
    x86_64-unknown-linux-gnu`. The GitHub Actions CI (`.github/workflows/build.yml`) derives the artifact name from
    `cargo metadata` package name, so it auto-produces `equibop-<triple>` matching the manifest `CodePaths` — no
    workflow edit needed.
- [ ] **T5.2 (V)** Document + verify the Equibop build.
  - Files: `README.md`. Do: reproducible steps to compile the plugin inside an Equibop/Equicord checkout. Deps: T3.*.
    Accept: plugin compiles and shows in the plugin list after restart.
- [~] **T5.3 (R+V)** MVP end-to-end test.
  - Do: install both sides; in OpenDeck verify (1) Toggle Mute, (2) Toggle Deafen, (3) Push to Mute, (4) Push to
    Talk, (5) Voice Channel join/leave, (6) Text Channel jump — all act in Equibop AND reflect button state;
    (7) muting in Discord UI updates buttons; (8) closing Equibop → `show_alert`; (9) reopening → reconnect +
    resync. Deps: T5.1,T5.2. Accept: all 9 checks pass. **← MVP milestone.**
  - → in progress: **checks (1)+(2) PASS — user-confirmed Toggle Mute + Toggle Deafen** drive Equibop and reflect
    button state (after the R deadlock fix `96cf759` + V finder fix `05b9974`). Both halves now build clean. Remaining
    user-side checks: (3) Push-to-Mute, (4) Push-to-Talk, (5) Voice Channel, (6) Text Channel, (7) Discord-UI→button
    feedback, (8) disconnect→show_alert, (9) reconnect+resync.

### Phase 6 — Tier-2 actions (Medium feasibility)
> **V-side handlers for all of T6.1–T6.5 are implemented and build-verified** in `equibop-opendeck` `e88aaec`
> (each finder grounded in Equicord source; `pnpm build` produces dist/equibop/renderer.js). The **R side already
> ships every command + feedback path** (protocol, actions, PI plumbing carried from the v0.5.0 baseline). Left `[~]`
> until live-tested; the targeted actions (T6.3/T6.4/T6.5) also need their R-side PI **picker** confirmed to populate
> from the cached `devices`/`soundboard`/`guilds` payloads (R sends them via `send_*_to_pi`; not yet exercised here).
- [~] **T6.1 (R+V)** Toggle Voice Input Mode — protocol `setVoiceInputMode`/`toggleVoiceInputMode`; Vencord
  media-engine `getMode`/`setMode`. Deps: T5.3. Accept: button flips Discord between PTT and Voice Activity.
  - → V: `MediaEngineStore.getMode()/getModeOptions()` + `findByProps("setMode","getModeOptions").setMode(ctx,mode,opts)`
    (re-passes current ModeOptions). No PI needed. ⚠ setter signature is medium-confidence — VERIFY in DevTools.
- [~] **T6.2 (R+V)** Volume Control (dial/encoder) — `setInputVolume`/`setOutputVolume` + `devices` feedback;
  handle `dialRotate`. Deps: T5.3. Accept: turning the dial changes Discord input/output volume; PI shows current.
  - → V: FluxDispatcher `AUDIO_SET_INPUT_VOLUME` (0-100) / `AUDIO_SET_OUTPUT_VOLUME` (0-200), matching `vcPanelSettings`.
- [~] **T6.3 (R+V)** User Volume Control — `setUserVolume{userId,value}`; Vencord `setLocalVolume`; PI picks a
  user. Deps: T5.3. Accept: changes a specific user's local output volume.
  - → V: `findByProps("setLocalVolume").setLocalVolume(userId,value)` (2-arg per discord-types). Needs PI user-picker.
- [~] **T6.4 (R+V)** Set Audio Device — `setInputDevice`/`setOutputDevice` + `devices` list; PI device picker.
  Deps: T5.3. Accept: switches Discord input/output device.
  - → V: `AUDIO_SET_INPUT_DEVICE`/`AUDIO_SET_OUTPUT_DEVICE {id}`; `buildDevices()` enumerates get{Input,Output}Devices.
- [~] **T6.5 (R+V)** Soundboard — `playSoundboard{guildId,soundId}` + `soundboard` list from `SoundboardStore`;
  PI sound picker. Deps: T5.3. Accept: plays a soundboard sound in the active call.
  - → V: `RestAPI.post SEND_SOUNDBOARD_SOUND(voiceChannel){sound_id,source_guild_id}` (matches `exitSounds`);
    `buildSoundboard()` flattens `SoundboardStore.getSounds()`.

### Phase 7 — Tier-3 actions (Hard/Uncertain) + final rebrand
- [~] **T7.1 (R+V)** Toggle Video / camera — `setVideo`/`toggleVideo`; verify the camera-toggle action exists in
  Equicord. Deps: T5.3. Accept: toggles camera; if infeasible, mark `[!]` with findings.
  - → V (`e88aaec`): `FluxDispatcher.dispatch({type:"MEDIA_ENGINE_SET_VIDEO_ENABLED", enabled})` with
    `findByProps("isVideoEnabled").isVideoEnabled()` for state — **exact mechanism matches the shipping
    `toggleVideoBind` + `randomVoice` plugins** (high confidence). Build-verified; awaiting live test.
- [~] **T7.2 (R+V)** Notifications — observe Flux `MESSAGE_CREATE`/notification dispatch → `notification` event;
  action surfaces/opens the channel. Deps: T5.3. Accept: incoming notifications drive the key; or `[!]` with findings.
  - → V (`e88aaec`): forwards Discord's `NOTIFICATION_CREATE` flux (respects mute/DnD) as
    `{type:"notification", channelId, title, body}`; payload shape confirmed via `silenceUsers` (`event.message`) and
    `orbolayBridge` `handleMessageNotification` (`dispatch.message.channel_id`). R caches channel_id → updates the
    Notifications button counter. Build-verified; awaiting live test.
- [~] **T7.3 (R+V)** Toggle Screen Share / Go Live — **research first**: can a renderer plugin start Go Live
  without the source picker, and how does Equibop's Linux screenshare path interact? Implement or document
  why not. Deps: T5.3. Accept: starts/stops Go Live, OR a written infeasibility note + fallback (e.g. trigger
  the picker).
  - → **RESEARCH RESOLVED + implemented (V `e88aaec`).** Programmatic Go Live IS feasible, reusing exactly what
    Equicord's own `InstantScreenshare` does: START via `startStream = findByCodeLazy('type:"STREAM_START"')` called
    `startStream(guildId, channelId, {pid:null, sourceId, sourceName, audioSourceId, sound, previewDisabled})`, where
    the source comes from `getDesktopSources = findByCodeLazy("desktop sources")`; STOP via
    `stopStream = findByCodeLazy('type:"STREAM_STOP"')` with the key `guild:<g>:<c>:<owner>` / `call:<c>:<owner>`
    (format confirmed against `orbolayBridge`); active state from `ApplicationStreamingStore.getCurrentUserActiveStream()`.
    Preconditions (in a voice channel, STREAM permission, not a stage) are checked → `console.warn` not throw.
    **Caveat (the original open question): on Wayland, starting pops the OS screen-source portal picker** — this is a
    Wayland security requirement (the same portal Discord's own "Share Your Screen" triggers) and cannot be bypassed
    from a renderer plugin. **Stop is fully headless.** On X11 unattended start may work. So: not a pure
    one-press-no-dialog start on Wayland, but the button genuinely starts/stops Go Live. Build-verified; awaiting live test.
  - → **DEEP-DIVE VERIFIED + HARDENED (V, session 4)** against current Equibop `main` (`src/main/screenShare.ts`,
    `src/main/constants.ts`, `src/renderer/patches/screenShareFixes.ts`) via a 3-agent research workflow. Confirmed the
    exact flow: our `startStream` → Discord `getDisplayMedia` → Equibop's main-process `setDisplayMediaRequestHandler`
    → `desktopCapturer.getSources`. **On Wayland that `getSources` (inside Chromium) is what pops the KDE portal**;
    Equibop then auto-takes `sources[0]` and SUPPRESSES its own picker (`skipPicker:true`) — **our `sourceId` is
    ignored on Wayland** (only honoured on the X11 branch, which shows Equibop's *own* in-app picker, `skipPicker:false`).
    **No `restore_token`/`persist_mode` exists in Equibop, Vesktop, or Electron's getDisplayMedia path** (zero matches),
    so a dialog-free one-time-grant is NOT achievable from a userplugin; the only dialog-free route is X11/XWayland,
    which merely swaps the OS portal for Equibop's in-app picker (also not headless). **Conclusion: on this user's
    KDE/Wayland, "works to some extent" = one-tap start + one OS-dialog confirmation; headless stop. This is the
    ceiling for a renderer plugin.** Hardening shipped: (R1) post-start reconcile — after `startStream`, a ~3 s timer
    pushes ground-truth state so a *cancelled* portal clears a stuck "live" button; (R2) `previewDisabled` now reads
    the user's real `voiceAndVideo/disableStreamPreviews` setting via `getUserSettingLazy` instead of hardcoding
    `false`; (R3) `screenShareStarting` in-flight guard debounces double-taps during the dialog window (cleared on
    `STREAM_CREATE`/`STREAM_DELETE`); (R4) `enumerateSources` retries the 3-arg `getDesktopSources` form on empty and
    prefers a `screen:`-type source. Build-verified (`pnpm build` green).
  - → **ROOT-CAUSE FOUND + FIXED (V, session 4 cont — live-debugged with the user). Screen share now WORKS.**
    The button did nothing because the renderer pre-enumeration step threw and the code bailed before ever calling
    `startStream`. DevTools showed: `getDesktopSources threw: Invariant Violation: "Can't get desktop sources outside
    of native app"`. **Equibop runs Discord's WEB client, where Discord's own `getDesktopSources` is hard-gated to the
    official native app and throws** — so the entire instantScreenshare blueprint (enumerate in renderer → `startStream`
    with a real id) is impossible here, and InstantScreenshare itself can't work on Equibop. Confirmed against the
    INSTALLED `/usr/lib/equibop/app.asar` (extracted + read): Equibop's real screen share never calls that function —
    it relies on the renderer calling `getDisplayMedia`, which the main-process `setDisplayMediaRequestHandler` services
    by running `desktopCapturer.getSources` ITSELF and (Wayland) auto-capturing `sources[0]` with `skipPicker:true`.
    **So the `sourceId` is irrelevant — the handler always re-enumerates.** FIX: stop calling `getDesktopSources`
    entirely; dispatch `startStream(guild, channel, {pid:null, sourceId:"screen:0:0", sourceName:"Screen",
    audioSourceId:"Screen", sound, previewDisabled})` with a placeholder id and let Equibop's handler capture. **Verified
    LIVE in the user's Equibop console — `startStream` with the placeholder id goes Go-Live and shares the screen
    ("WORKING!").** On Wayland Equibop auto-picks the primary/active screen, so this is effectively the user's requested
    "share the active screen." Removed `enumerateSources`/`getDesktopSources`/`IS_WINDOWS`/`getMediaEngine`; kept the
    R1/R3 reconcile+debounce hardening. Build-verified (`pnpm build` green). **Supersedes the session-4 "one OS-dialog
    confirmation is the ceiling" conclusion** — the dialog was never the blocker; the throwing enumerator was.
  - → **BLACK-SCREEN ROOT CAUSE + REAL FIX (V `55a6e94`, session 4 cont — reverse-engineered Discord's web bundle,
    verified live).** After the enumerator fix the button went *live* but streamed BLACK. Cause: on Equibop's WEB
    client, Go Live is **capture-driven, not sourceId-driven** — `startStream` only does signaling; the video source
    must be a `getDisplayMedia` MediaStream that Discord binds into `connection.input.stream`, and there is NO public
    setter to inject a self-captured track (proved `replaceTrack` onto the WebRTC sender yields `no outbound-rtp` — the
    engine gates encoding on its own capture state). Fetched + grepped the live Discord bundle
    (`discord.com/assets/web.<hash>.js`) to find the genuine web path and confirmed each call against the user's runtime
    via DevTools introspection: **(1)** `startStream` → go-live connection; **(2)**
    `MediaEngineStore.getMediaEngine().desktopInputPool.acquire({width,height}, audio)` → calls `navigator.getDisplayMedia`
    (Equibop's main-process `setDisplayMediaRequestHandler` pops the KDE portal on Wayland) → a capture wrapper; **(3)**
    `connection.setDesktopInput(capture)` on the go-live connection (`streamUserId===userId`) — exactly Discord's own
    `MEDIA_ENGINE_SET_GO_LIVE_SOURCE` handler (`i=desktopInputPool.get(id); eachConnection(...).setDesktopInput(i)`).
    Live result: **frames flowing** (`framesSent` climbing, real WxH, `EquibopStreamFixes Applied constraints` — identical
    to a normal share; `bytesSent:0` only because no viewer). Other facts pinned: Discord's web `getDesktopSources`
    throws "Can't get desktop sources outside of native app"; Equibop must run **Wayland ozone** (`equibop --wayland` /
    `~/.config/equibop-flags.conf`) or capture is black via XWayland. Plugin now does startStream → acquire → setDesktopInput
    and releases the capture on stop. **OPEN:** whether a deck-triggered (WS, no renderer user-gesture) `getDisplayMedia`
    is allowed — relying on Equibop's `setDisplayMediaRequestHandler` relaxing transient-activation; confirm on live button.
- [x] **T7.4 (R)** Final rebrand/UUIDs — new UUID namespace (e.g. `com.garrettfaucher.equibop.<suffix>`) across
  `src/actions/*` and `assets/manifest.json` (`Name`,`Author`,`CodePaths`, all action UUIDs); swap icons if
  desired. Deps: parity actions done. Accept: manifest UUIDs match Rust; plugin loads under the new identity.
  - → done: identity rebranded `me.amankhanna.oadiscord` → **`com.garrettfaucher.equibop`** (matches the user's other
    plugins). All 14 action UUIDs renamed in `src/actions/*` AND `assets/manifest.json` (**verified exact 14/14 match**);
    manifest `Name`→"Equibop", `Author`→"Garrett Faucher", `Category`→"Equibop", binary base `oadiscord-*`→`equibop-*`.
    Built + installed to `~/.config/opendeck/plugins/com.garrettfaucher.equibop.sdPlugin/`; **old plugin folder + stale
    settings removed and the old running process killed** (port 6789 freed). ⚠️ **User chose to orphan existing buttons**
    — the placed Discord buttons in profile `Default.json` reference the old UUIDs and must be re-added under the new
    "Equibop" category after an OpenDeck restart. Icons unchanged (kept the existing art).

### Phase 8 — Polish
- [ ] **T8.1** Reconnection/port-change/edge-case hardening (port change restarts server + client reconnects;
  `channelId:null` when not in voice; rapid PTT no desync — echoed `stateUpdate` corrects optimistic state).
- [ ] **T8.2** READMEs (both repos): architecture diagram, install, troubleshooting, link PROTOCOL.md + PLAN.md.
- [ ] **T8.3** Release: tag, build artifacts for all targets, publish.

---

## Risks & open questions
- **Go Live (T7.3) — RESOLVED.** Programmatic start/stop works via Discord's `STREAM_START`/`STREAM_STOP` action
  creators (blueprint: Equicord `InstantScreenshare`). The only residual: on **Wayland**, starting pops the OS
  source-picker portal (unavoidable; same portal as Discord's native "Share Your Screen"); stop is headless. Implemented.
- **Exact Vencord finder names** for media-engine setters — now **resolved from local Equicord source**, not guessed:
  mode (`findByProps("setMode","getModeOptions")`, med-confidence on arg order), volume/device (FluxDispatcher
  `AUDIO_SET_*`, matches `vcPanelSettings`), local-volume (`setLocalVolume`, 2-arg per discord-types), video
  (`MEDIA_ENGINE_SET_VIDEO_ENABLED`, matches `toggleVideoBind`). All build-verified; the medium-confidence ones carry
  `VERIFY` comments for a final DevTools confirmation during live testing.
- **Build-from-source friction (V)** is unavoidable (no runtime plugin load). Must be documented; consider the
  full-Equibop-fork option later if distribution is painful.
- **Upstream drift**: 0.5.0 is the baseline; if upstream releases more actions, re-evaluate parity scope.
- **Single client assumption**: the WS server tracks one Equibop client. If multiple Equibop windows/instances
  connect, define behavior (last-wins is fine for v1).

## Decisions confirmed with user
- Equibop side: **userplugin now, full fork later**.
- Scope: **0.5.0 parity, core-first** (user confirmed 0.5.0 includes the streaming actions they remembered).
- Task log home: **`PLAN.md` committed in `streamdeck-equibop`** (this file).

---

## Reference appendix (so future sessions don't re-research)

### Upstream plugin (`OpenActionPlugins/discord`, v0.5.0, branch `main`)
- Source files: `src/main.rs`, `src/client.rs`, `src/oauth.rs`, `src/rpc_events.rs`, `src/cache.rs`,
  `src/actions/{voice_settings.rs,channel.rs,notifications.rs,video.rs,screen_share.rs,soundboard.rs}`,
  `src/actions/voice_settings/{audio_device_utils.rs,set_audio_device.rs,user_volume_control.rs,volume_control.rs}`.
- IPC layer: `discord-ipc-rust` (`github.com/nekename/discord-ipc-rust`) — connects to `discord-ipc-0`.
- Commands used (`SentCommand`): `Authorize`, `GetVoiceSettings`, `SetVoiceSettings`, `GetGuilds`,
  `GetSelectedVoiceChannel`, `GetSoundboardSounds`, `PlaySoundboardSound{guild_id,sound_id}`,
  `Subscribe/Unsubscribe`. Subscriptions: `VoiceSettingsUpdate`, `VideoStateUpdate`, `ScreenshareStateUpdate`,
  `VoiceChannelSelect`, `NotificationCreate`, `VoiceStateCreate/Update/Delete{channel_id}`.
- OAuth scopes (port needs none): `rpc, rpc.voice.read, rpc.voice.write, rpc.video.read, rpc.video.write,
  rpc.screenshare.read, rpc.screenshare.write, rpc.notifications.read, identify`.
- `cache.rs` caches guilds (`GuildStore`), soundboard sounds (`SoundboardStore`), notifications (Flux msgs) —
  all readable from Vencord Flux stores in the port, so most caching can be dropped.

### OpenDeck / OpenAction SDK
- OpenDeck app: `github.com/nekename/OpenDeck` (current ~v2.12.x; local plugin synced PI to v2.10.0). Plugins
  install to `~/.config/opendeck/plugins/<id>.sdPlugin/`. Flatpak prefix `~/.var/app/me.amankhanna.opendeck/`.
- Rust SDK crate `openaction` (`github.com/OpenActionAPI/rust`, docs.rs/openaction). `Action` trait:
  `const UUID`, `type Settings`, async `key_down/key_up/will_appear/will_disappear/dial_rotate`. `Instance`:
  `set_state(u16)`,`set_image`,`set_title`,`show_alert`,`show_ok`,`send_to_property_inspector`,`set_settings`.
  `GlobalEventHandler`: `plugin_ready()`,`did_receive_global_settings()`. Entry: `run(env::args().collect())`.
- PI SDK: `@openaction/svelte-pi` (`globalSettings` store, `openUrl`). Plugin handshake uses
  `-port -pluginUUID -registerEvent -info`; protocol is Stream Deck-SDK compatible.
- Manifest (`assets/manifest.json`): `Name,Author,Version,Category,Icon,OS,CodePaths,Actions[]` with
  `UUID,Name,Tooltip,States[],Controllers,PropertyInspectorPath,DisableAutomaticStates,SupportedInMultiActions`.

### Equibop / Equicord / Vencord
- Repos: Equibop `github.com/Equicord/Equibop` (Vesktop fork, bundles Equicord); Equicord
  `github.com/Equicord/Equicord` (Vencord fork); Vencord `github.com/Vencord/Vencord`; Vesktop
  `github.com/Vencord/Vesktop`; arRPC `github.com/OpenAsar/arrpc` (rich-presence only; mute/deafen = open
  issue #79).
- Process model: **main** (Node, full access — can host sockets/servers) · **preload** (contextBridge) ·
  **renderer** (Discord web + Equicord; WebSocket *client* only, no server). Hence Rust=server, plugin=client.
- Userplugins live in `src/userplugins/<name>/index.tsx`; require `pnpm i && pnpm build` + restart (no hot
  load). Equicord plugin docs: `docs.equicord.org/plugins`; Vencord custom plugins:
  `docs.vencord.dev/installing/custom-plugins/`.
- `definePlugin({name,description,authors,start(){},stop(){},flux:{EVENT(){}}})`. Webpack finders:
  `findByProps`,`findByPropsLazy`,`findByCode`,`findStoreLazy`; common stores via `@webpack/common`.
- Confirmed Vencord references to model from:
  - WS client template: Vencord `src/plugins/arRPC.web/index.tsx`.
  - Voice state + flux events + store reads: Vencord `src/plugins/vcNarrator/index.tsx` — uses
    `MediaEngineStore.isSelfMute()/isSelfDeaf()`, `SelectedChannelStore.getVoiceChannelId()`,
    `VoiceStateStore.getVoiceStateForChannel(id)`, and flux `AUDIO_TOGGLE_SELF_MUTE`,`AUDIO_TOGGLE_SELF_DEAF`,
    `VOICE_STATE_UPDATES`.
  - Join/leave voice: Equicord `src/equicordplugins/followVoiceUser/index.tsx` —
    `findByPropsLazy("selectVoiceChannel").selectVoiceChannel(id|null)`.
  - Type surface: Vencord `packages/discord-types/src/stores/MediaEngineStore.d.ts`
    (`isSelfMute/isSelfDeaf/setSelfMute/setSelfDeaf` and related).

### Changelog (append as the plan evolves)
- _init_ — plan created from research; scope corrected from stale 4-action v0.2.2 to upstream 14-action v0.5.0;
  WS-bridge architecture chosen; user decisions recorded.
- _session 1 (Phase 0)_ — `main` fast-forwarded to v0.5.0 baseline; branch `equibop-port` created;
  `PROTOCOL.md` (v1) + `PLAN.md` added to the repo. T0.1, T0.2 done; T0.3 baseline done (rename deferred).
  Next up: T0.4 (scaffold the Equibop userplugin repo) and Phase 1 (Rust WS-server refactor).
- _session 2 (T0.4 + Phases 1–2 + T4.1)_ — **R side fully refactored off Discord-RPC onto the WS bridge and
  builds clean (release, 0 warnings; 7 protocol tests pass); the property inspector is reworked and builds
  (`svelte-check` 0 errors).** New modules `protocol.rs`/`ws_server.rs`/`feedback.rs`/`state.rs`;
  `oauth.rs`/`client.rs`/`rpc_events.rs` deleted; `Cargo.toml` deps swapped. **All 14** action handlers now route
  through `send_command` (MVP 6 + the Tier-2/3 Rust side ported early so the tree compiles). `PROTOCOL.md` wire
  encodings clarified. T0.4 scaffolded the **`equibop-opendeck`** repo. An adversarial review workflow
  (4 dimensions + verifier passes) vetted the refactor and confirmed 2 issues: (1) a `restart_server` port-rebind
  race — **fixed** (bind now drains the old listener via `abort()`+`await` before rebinding and surfaces bind
  failures to the PI via `set_bridge_error`); (2) the channel-PI silent breakage — **fixed in T4.1** (the PI now
  reads nested `guild.voice`/`guild.text` and drops the dead `request_channels` round-trip; `ApplicationSettings`
  swapped from clientId/secret/OAuth to a Bridge Port field + error banner + install block). Done this session:
  **T0.4, T1.1–T1.5, T2.1–T2.2, T4.1**. Still pending: **T0.3 rename** (manual), **T4.2** (deferred — batched with
  the coordinated rename), and all **V-side handlers** (Phase 3 onward). Next up: **Phase 3** (the userplugin WS
  client — the only remaining piece for an MVP end-to-end test) and then **Phase 5** (MVP e2e).
- _session 3 (MVP fix + full V-side parity)_ — **Diagnosed why no Discord button worked: a lock-ordering deadlock
  in `main.rs`** (the settings read-guard was held across `restart_server()`, which takes the write lock) so the
  plugin served the bridge but never reached `run()` to connect to OpenDeck → no willAppear/keyUp. **Fixed** (`96cf759`,
  read the port into a local first) + the V finder fix (`equibop-opendeck` `05b9974`). **User confirmed Toggle Mute +
  Toggle Deafen now drive Equibop** (T5.3 checks 1–2). Then **implemented every remaining V-side command**
  (`equibop-opendeck` `e88aaec`) — T6.1–T6.5, T7.1–T7.3 — via an 8-domain source-research workflow against the local
  Equicord checkout (finders grounded in `vcPanelSettings`/`exitSounds`/`toggleVideoBind`/`instantScreenshare`/
  `orbolayBridge`, not guessed) + synthesis; the plugin **compiles clean into dist/equibop/renderer.js** (`pnpm build`).
  **T7.3 Go Live research resolved**: feasible via `STREAM_START`/`STREAM_STOP`; Wayland pops the OS source picker on
  start (unavoidable), stop is headless. The **R side needed no changes** (already complete). Marked T3.1/T3.2 `[x]`,
  T3.3/T3.4/T5.3/T6.1–T6.5/T7.1–T7.3 `[~]` (built + source-verified, awaiting the user's live test).
- _session 3 (cont.) — identity rebrand_ — at the user's go-ahead (they chose to **orphan their existing buttons**),
  did the full **T7.4 rebrand**: `me.amankhanna.oadiscord`→**`com.garrettfaucher.equibop`** (all 14 action UUIDs,
  verified 14/14 manifest↔Rust match), binary/package `oadiscord`→`equibop`, PI package `oadiscord-pi`→`equibop-pi`
  (T4.2), manifest Name/Author/Category→Equibop / Garrett Faucher / Equibop, README + build.sh usage updated. Built +
  installed to `com.garrettfaucher.equibop.sdPlugin`; removed the old plugin folder + stale settings + killed the old
  process. CI needs no edit (artifact name is `cargo metadata`-driven). **T7.4/T4.2/T5.1 → `[x]`; T0.3 in-repo identity
  done** (only the GitHub-repo/local-dir *names* remain — a manual user step). Still open: **T5.2** (V build docs),
  **T8.x** polish, and live-test promotion of the `[~]` action tasks. **User must restart OpenDeck** to load the
  renamed plugin and re-add buttons under the new "Equibop" category.
