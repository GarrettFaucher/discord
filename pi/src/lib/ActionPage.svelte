<script lang="ts">
	import { actionSettings } from "@openaction/svelte-pi";
	import ApplicationSettings from "$lib/ApplicationSettings.svelte";
	import EquibopPanel from "$lib/EquibopPanel.svelte";

	export let kind: "mic" | "deafen";
	export let pushMode: boolean;

	$: backend = ($actionSettings as any)?.backend ?? "discord";

	function setBackend(next: "discord" | "equibop") {
		$actionSettings = { ...($actionSettings ?? {}), backend: next };
	}
</script>

<div
	class="mb-3 inline-flex rounded-lg border border-neutral-600 bg-neutral-800 p-0.5 text-xs"
>
	<button
		class="cursor-pointer rounded-md px-3 py-1 {backend === 'discord'
			? 'bg-neutral-600 text-white'
			: 'text-neutral-400 hover:bg-neutral-700 hover:text-neutral-200'}"
		on:click={() => setBackend("discord")}
	>
		Native Discord
	</button>
	<button
		class="cursor-pointer rounded-md px-3 py-1 {backend === 'equibop'
			? 'bg-neutral-600 text-white'
			: 'text-neutral-400 hover:bg-neutral-700 hover:text-neutral-200'}"
		on:click={() => setBackend("equibop")}
	>
		Equibop
	</button>
</div>

{#if backend === "equibop"}
	<EquibopPanel {kind} {pushMode} />
{:else}
	<ApplicationSettings />
{/if}
