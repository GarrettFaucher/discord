<script lang="ts">
	import {
		actionSettings,
		actionInfo,
		eventTarget,
	} from "@openaction/svelte-pi";

	import ApplicationSettings from "$lib/ApplicationSettings.svelte";

	interface Channel {
		id: string;
		name: string;
	}

	interface Guild {
		id: string;
		name: string;
		voice: Channel[];
		text: Channel[];
	}

	let guilds: Guild[] = $state([]);

	// This PI is shared by the Voice Channel and Text Channel actions; the action UUID decides which
	// nested channel list (voice vs text) of each guild to show.
	let isVoice = $derived(($actionInfo?.action ?? "").endsWith("voicechannel"));

	let selectedGuild = $derived($actionSettings.guild_id ?? "");
	let selectedChannel = $derived($actionSettings.channel_id ?? "");

	let channels: Channel[] = $derived.by(() => {
		const guild = guilds.find((g) => g.id === selectedGuild);
		if (!guild) return [];
		return (isVoice ? guild.voice : guild.text) ?? [];
	});

	// The server now sends the full guild list with nested voice/text channels, so there is no longer
	// a per-guild channel request round-trip.
	eventTarget.addEventListener("sendToPropertyInspector", (event: any) => {
		const payload = event.detail?.payload ?? {};

		if (Array.isArray(payload.guilds)) {
			guilds = payload.guilds;
			if (
				(!selectedGuild || !guilds.some((g) => g.id === selectedGuild)) &&
				guilds.length > 0
			) {
				$actionSettings = {
					...$actionSettings,
					guild_id: guilds[0].id,
					channel_id: "",
				};
			}
		}
	});

	// Default the channel to the first available when none is selected (or the selection is stale).
	$effect(() => {
		if (
			channels.length > 0 &&
			(!selectedChannel || !channels.some((c) => c.id === selectedChannel))
		) {
			$actionSettings = { ...$actionSettings, channel_id: channels[0].id };
		}
	});

	function updateGuild(event: Event) {
		const guild_id = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, guild_id, channel_id: "" };
	}

	function updateChannel(event: Event) {
		const channel_id = (event.target as HTMLSelectElement).value;
		$actionSettings = { ...$actionSettings, channel_id };
	}
</script>

<div class="space-y-4 text-neutral-200">
	<div class="grid grid-cols-[250px_1fr] items-center">
		<label for="guild" class="text-sm">Server</label>
		<div class="select-wrapper">
			<select
				id="guild"
				value={selectedGuild}
				onchange={updateGuild}
				class="w-full"
			>
				{#if guilds.length === 0}
					<option value="" disabled>No servers available</option>
				{:else}
					<option value="" disabled>Select a server</option>
					{#each guilds as guild}
						<option value={guild.id}>{guild.name}</option>
					{/each}
				{/if}
			</select>
		</div>
	</div>

	<div class="grid grid-cols-[250px_1fr] items-center">
		<label for="channel" class="text-sm">
			{isVoice ? "Voice channel" : "Text channel"}
		</label>
		<div class="select-wrapper">
			<select
				id="channel"
				value={selectedChannel}
				onchange={updateChannel}
				class="w-full"
				disabled={!selectedGuild}
			>
				{#if !selectedGuild}
					<option value="" disabled>Select a server first</option>
				{:else if channels.length === 0}
					<option value="" disabled>No channels available</option>
				{:else}
					<option value="" disabled>Select a channel</option>
					{#each channels as channel}
						<option value={channel.id}>{channel.name}</option>
					{/each}
				{/if}
			</select>
		</div>
	</div>
</div>

<hr class="my-4 border-neutral-700" />

<ApplicationSettings />
