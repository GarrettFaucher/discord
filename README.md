## streamdeck-equibop

An [OpenDeck](https://github.com/nekename/OpenDeck) plugin for controlling **[Equibop](https://github.com/Equicord/Equibop)** (a privacy-focused Discord client bundling Equicord/Vencord) from a stream deck. Unlike the original Discord plugin — which drives the official client over the Discord RPC IPC socket with OAuth — this fork talks to a companion **[EquibopOpenDeck userplugin](https://github.com/GarrettFaucher/equibop-opendeck)** over a local WebSocket bridge, so it needs no OAuth, client ID/secret, or `rpc.*` scopes. See `PLAN.md` and `PROTOCOL.md`.

> [!WARNING]
> This plugin is in an alpha state. It requires the companion EquibopOpenDeck userplugin to be built into your Equibop/Equicord install (no runtime plugin loading — see that repo's README).

#### Actions

- Toggle Mute
- Toggle Deafen
- Push to Mute
- Push to Talk
- Toggle Voice Input Mode
- Toggle Video
- Toggle Screen Share
- Volume Control
- User Volume Control
- Set Audio Device
- Text Channel
- Voice Channel
- Soundboard
- Notifications
