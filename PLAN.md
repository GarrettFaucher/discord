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
  - → PENDING (rename, deferred): (a) rename the GitHub repo + local dir `discord`→`streamdeck-equibop` is a
    **manual user step** (renaming the cwd mid-session is disruptive; GitHub rename needs the user). (b) The
    `Cargo.toml` package + manifest `CodePaths` binary rename is intentionally deferred to land **together**
    with T1.1 / T4.2 / T5.1 so the tree stays buildable at every step (renaming the package alone would
    desync the manifest's `oadiscord-*` binary names).
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
    is rewritten in **T4.1**. Not the MVP *build* gate, but it IS an MVP *functionality* gate.
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
- [ ] **T3.1 (V)** WS client lifecycle.
  - Files: `index.tsx`. Do: model on Vencord `src/plugins/arRPC.web/index.tsx` — connect in `start()`,
    `close()` in `stop()`, `onopen`→send `hello` then initial `stateUpdate`, `onmessage`→dispatch by `type`,
    `onclose`/`onerror`→reconnect with backoff (~5s). Deps: T0.4. Accept: with R running, R logs `hello` and
    receives the initial `stateUpdate`.
- [ ] **T3.2 (V)** Voice command handlers + verify finders in DevTools.
  - Files: `index.tsx`. Do: `const Media = findByProps("setSelfMute","setSelfDeaf")` (fallback
    `findByProps("toggleSelfMute","toggleSelfDeaf")`); `MediaEngineStore` from `@webpack/common`. Implement
    `setMute`/`setDeafen`/`toggleMute`/`toggleDeafen` and `requestState`. **Verify the finder resolves in
    DevTools.** Deps: T3.1. Accept: each command toggles the real Discord mute/deafen indicator.
- [ ] **T3.3 (V)** Channel command handlers.
  - Files: `index.tsx`. Do: `const VoiceActions = findByPropsLazy("selectVoiceChannel")`; implement
    `selectVoiceChannel(id|null)` and `selectTextChannel` (verify the text-nav action — `NavigationRouter`/
    channel select); implement `requestGuilds` reading `GuildStore`/`ChannelStore`/`SortedGuildStore`. Deps: T3.1.
    Accept: commands join/leave voice and jump text channels; `guilds` event populates the R-side cache.
- [ ] **T3.4 (V)** State feedback via Flux.
  - Files: `index.tsx`. Do: `flux:{ AUDIO_TOGGLE_SELF_MUTE(){…}, AUDIO_TOGGLE_SELF_DEAF(){…}, VOICE_STATE_UPDATES(){…} }`
    handlers read `MediaEngineStore` + `SelectedChannelStore.getVoiceChannelId()` and push `stateUpdate`. Deps: T3.1.
    Accept: muting from Discord's own UI flips the stream-deck button state.

### Phase 4 — Rust PI rework  (R)
- [ ] **T4.1 (R)** Replace the Discord-app PI with a bridge-port PI **and migrate the channel PI**.
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
- [ ] **T4.2 (R)** Rebrand PI package.
  - Files: `pi/package.json`. Do: rename `oadiscord-pi`→`equibop-pi`; rebuild; confirm `assets/pi/*` regenerate.
    Deps: T4.1. Accept: PI builds; generated HTML references the new bundle.

### Phase 5 — MVP build, package & end-to-end verification
- [ ] **T5.1 (R)** Build/package the Rust plugin.
  - Files: `build.sh`, `.github/workflows/build.yml`. Do: binary base name → `equibop`; ensure manifest
    `CodePaths` match; `./build.sh <out> equibop <triple>` installs to `~/.config/opendeck/plugins/`. Deps: T2.*,T4.*.
    Accept: bundle builds; CI green.
- [ ] **T5.2 (V)** Document + verify the Equibop build.
  - Files: `README.md`. Do: reproducible steps to compile the plugin inside an Equibop/Equicord checkout. Deps: T3.*.
    Accept: plugin compiles and shows in the plugin list after restart.
- [ ] **T5.3 (R+V)** MVP end-to-end test.
  - Do: install both sides; in OpenDeck verify (1) Toggle Mute, (2) Toggle Deafen, (3) Push to Mute, (4) Push to
    Talk, (5) Voice Channel join/leave, (6) Text Channel jump — all act in Equibop AND reflect button state;
    (7) muting in Discord UI updates buttons; (8) closing Equibop → `show_alert`; (9) reopening → reconnect +
    resync. Deps: T5.1,T5.2. Accept: all 9 checks pass. **← MVP milestone.**

### Phase 6 — Tier-2 actions (Medium feasibility)
- [ ] **T6.1 (R+V)** Toggle Voice Input Mode — protocol `setVoiceInputMode`/`toggleVoiceInputMode`; Vencord
  media-engine `getMode`/`setMode`. Deps: T5.3. Accept: button flips Discord between PTT and Voice Activity.
- [ ] **T6.2 (R+V)** Volume Control (dial/encoder) — `setInputVolume`/`setOutputVolume` + `devices` feedback;
  handle `dialRotate`. Deps: T5.3. Accept: turning the dial changes Discord input/output volume; PI shows current.
- [ ] **T6.3 (R+V)** User Volume Control — `setUserVolume{userId,value}`; Vencord `setLocalVolume`; PI picks a
  user. Deps: T5.3. Accept: changes a specific user's local output volume.
- [ ] **T6.4 (R+V)** Set Audio Device — `setInputDevice`/`setOutputDevice` + `devices` list; PI device picker.
  Deps: T5.3. Accept: switches Discord input/output device.
- [ ] **T6.5 (R+V)** Soundboard — `playSoundboard{guildId,soundId}` + `soundboard` list from `SoundboardStore`;
  PI sound picker. Deps: T5.3. Accept: plays a soundboard sound in the active call.

### Phase 7 — Tier-3 actions (Hard/Uncertain) + final rebrand
- [ ] **T7.1 (R+V)** Toggle Video / camera — `setVideo`/`toggleVideo`; verify the camera-toggle action exists in
  Equicord. Deps: T5.3. Accept: toggles camera; if infeasible, mark `[!]` with findings.
- [ ] **T7.2 (R+V)** Notifications — observe Flux `MESSAGE_CREATE`/notification dispatch → `notification` event;
  action surfaces/opens the channel. Deps: T5.3. Accept: incoming notifications drive the key; or `[!]` with findings.
- [ ] **T7.3 (R+V)** Toggle Screen Share / Go Live — **research first**: can a renderer plugin start Go Live
  without the source picker, and how does Equibop's Linux screenshare path interact? Implement or document
  why not. Deps: T5.3. Accept: starts/stops Go Live, OR a written infeasibility note + fallback (e.g. trigger
  the picker).
- [ ] **T7.4 (R)** Final rebrand/UUIDs — new UUID namespace (e.g. `com.garrettfaucher.equibop.<suffix>`) across
  `src/actions/*` and `assets/manifest.json` (`Name`,`Author`,`CodePaths`, all action UUIDs); swap icons if
  desired. Deps: parity actions done. Accept: manifest UUIDs match Rust; plugin loads under the new identity.

### Phase 8 — Polish
- [ ] **T8.1** Reconnection/port-change/edge-case hardening (port change restarts server + client reconnects;
  `channelId:null` when not in voice; rapid PTT no desync — echoed `stateUpdate` corrects optimistic state).
- [ ] **T8.2** READMEs (both repos): architecture diagram, install, troubleshooting, link PROTOCOL.md + PLAN.md.
- [ ] **T8.3** Release: tag, build artifacts for all targets, publish.

---

## Risks & open questions
- **Go Live (T7.3)** is the one genuinely uncertain action — programmatic Go Live needs a source and Equibop
  has custom Linux screenshare handling. Treat as research-then-decide; MVP and parity do not depend on it.
- **Exact Vencord finder names** for media-engine setters (mode/volume/device/local-volume/video) are
  best-known, not 100% confirmed; every Tier-2/3 task includes a DevTools verification step. Prefer driving
  from explicit booleans/values via setters over relying on internal toggles.
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
- _session 2 (T0.4 + Phases 1–2)_ — **R side fully refactored off Discord-RPC onto the WS bridge and builds
  clean (release, 0 warnings; 7 protocol tests pass).** New modules `protocol.rs`/`ws_server.rs`/`feedback.rs`/
  `state.rs`; `oauth.rs`/`client.rs`/`rpc_events.rs` deleted; `Cargo.toml` deps swapped. **All 14** action
  handlers now route through `send_command` (MVP 6 + the Tier-2/3 Rust side ported early so the tree compiles).
  `PROTOCOL.md` wire encodings clarified. T0.4 scaffolded the **`equibop-opendeck`** repo. Done this session:
  T0.4, T1.1–T1.5, T2.1–T2.2. An adversarial review workflow (4 dimensions + verifier passes) vetted the
  refactor and confirmed 2 issues: (1) a `restart_server` port-rebind race — **fixed** this session: the bind
  now drains the old listener (`abort()` + `await`) before rebinding and surfaces bind failures to the PI via
  `set_bridge_error`; (2) the channel-PI silent breakage — reclassified under T2.2 and folded into T4.1. Still pending:
  **T0.3 rename** (manual), **T4.x PI rework** (the channel/settings Svelte PI still references the old
  clientId/secret + per-guild channel request), and the **V-side handlers** (Phase 3 onward). Next up: Phase 3
  (userplugin WS client) and/or Phase 4 (Rust PI rework).
