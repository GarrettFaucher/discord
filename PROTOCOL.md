# streamdeck-equibop ⇄ Equibop bridge protocol — **v1**

This is the shared interface between the two repos. **Keep both sides in sync.** If you change anything
here, bump the version and update the Rust server (`streamdeck-equibop`) **and** the Equibop userplugin in
the same change.

- **Transport:** text JSON frames over `ws://127.0.0.1:<port>` (default **6789**).
- **Roles:** the Rust OpenDeck plugin (`streamdeck-equibop`) is the **server**; the Equibop/Vencord
  userplugin is the **client**.
- **Every message has a `type` field.** Unknown `type`s **must be ignored** (forward-compatibility).
- **Connection lifecycle:**
  1. On connect, the **client** sends `hello`.
  2. The **server** replies `ready`.
  3. The **client** immediately sends a full `stateUpdate` (and, when later asked via `request*`, the
     matching `devices` / `guilds` / `soundboard` event).
  4. On client disconnect, the **server** treats itself as "no client connected" — action handlers then
     call `show_alert()`.
  5. The **client** auto-reconnects with backoff (~5s) on close/error.

## Client → Server (events / feedback)

| `type` | fields | meaning |
|---|---|---|
| `hello` | `client` (string), `version` (number) | client announced itself |
| `stateUpdate` | `mute` (bool), `deaf` (bool), `inputMode` (`"PUSH_TO_TALK"` \| `"VOICE_ACTIVITY"`), `video` (bool), `screenshare` (bool), `channelId` (string \| null) | full self voice/video state for button feedback |
| `devices` | `input` (`[{id,name}]`), `output` (`[{id,name}]`), `currentInput` (string), `currentOutput` (string), `inputVolume` (0–100), `outputVolume` (0–200) | populates Volume / Set-Audio-Device PIs |
| `guilds` | `[{id,name,voice:[{id,name}],text:[{id,name}]}]` | populates Text / Voice Channel PIs |
| `soundboard` | `[{guildId,soundId,name,emojiName?}]` | populates Soundboard PI |
| `notification` | `channelId` (string), `title?` (string), `body?` (string) | a Discord notification arrived |

### Examples (V → R)
```json
{ "type": "hello", "client": "equibop-opendeck", "version": 1 }
```
```json
{ "type": "stateUpdate", "mute": true, "deaf": false, "inputMode": "VOICE_ACTIVITY",
  "video": false, "screenshare": false, "channelId": "123456789012345678" }
```

## Server → Client (commands)

| `type` | fields | maps to (Vencord) |
|---|---|---|
| `ready` | — | handshake reply |
| `requestState` / `requestDevices` / `requestGuilds` / `requestSoundboard` | — | client responds with the matching event |
| `setMute` | `value` (bool) | `setSelfMute(value)` |
| `setDeafen` | `value` (bool) | `setSelfDeaf(value)` |
| `toggleMute` / `toggleDeafen` | — | read store, call setter with negation |
| `setVoiceInputMode` | `mode` (string) | media-engine `setMode(...)` |
| `toggleVoiceInputMode` | — | read current mode, set the other |
| `setInputVolume` / `setOutputVolume` | `value` (number) | media-engine volume setters |
| `setUserVolume` | `userId` (string), `value` (number) | `setLocalVolume(userId, value, ...)` |
| `setInputDevice` / `setOutputDevice` | `deviceId` (string) | media-engine device setters |
| `setVideo` / `toggleVideo` | `value?` (bool) | camera enable / toggle action |
| `setScreenShare` / `toggleScreenShare` | `value?` (bool) | Go Live start/stop (**hard — see PLAN risks**) |
| `selectVoiceChannel` | `channelId` (string \| null) | `selectVoiceChannel(channelId)` |
| `selectTextChannel` | `guildId` (string), `channelId` (string) | navigate / transition to channel |
| `playSoundboard` | `guildId` (string), `soundId` (string) | soundboard play action |

### Examples (R → V)
```json
{ "type": "ready" }
```
```json
{ "type": "setMute", "value": true }
```
```json
{ "type": "selectVoiceChannel", "channelId": "123456789012345678" }
```

## Semantics notes
- `setMute` / `setDeafen` are **absolute** (used by Push-to-Mute / Push-to-Talk): `setSelfMute(value)` /
  `setSelfDeaf(value)`.
- `toggleMute` / `toggleDeafen` flip current state (used by the Toggle actions); the client computes the new
  value from `MediaEngineStore`.
- `requestState` → the client replies with a `stateUpdate`.
- The client pushes a fresh `stateUpdate` on every Flux `AUDIO_TOGGLE_SELF_MUTE`, `AUDIO_TOGGLE_SELF_DEAF`,
  and `VOICE_STATE_UPDATES` so button state tracks changes made from Discord's own UI.

## MVP subset
The first working milestone only needs: `hello`, `ready`, `stateUpdate`, `requestState`, `setMute`,
`setDeafen`, `toggleMute`, `toggleDeafen`, `selectVoiceChannel`, `selectTextChannel`, `guilds`. Everything
else lands in later phases (see `PLAN.md`).
