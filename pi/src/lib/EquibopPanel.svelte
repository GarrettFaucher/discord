<script lang="ts">
	import { globalSettings, sendToPlugin } from "@openaction/svelte-pi";

	export let kind: "mic" | "deafen";
	export let pushMode: boolean;

	$: state =
		kind === "mic"
			? Boolean(($globalSettings as any)?.equibopMicMuted)
			: Boolean(($globalSettings as any)?.equibopDeafened);

	function flip() {
		sendToPlugin({ action: "flip" });
	}

	$: statusLabel =
		kind === "mic"
			? state
				? "Mic state: Muted"
				: "Mic state: Unmuted"
			: state
				? "Deafened: Yes"
				: "Deafened: No";
</script>

<h2 class="mb-3 text-sm font-semibold text-neutral-100">Equibop Mode</h2>

<div
	class="mb-3 rounded-lg border border-neutral-600 bg-neutral-700 p-2 text-xs text-neutral-200"
>
	{statusLabel}
</div>

<button
	on:click={flip}
	class="cursor-pointer rounded-lg border border-neutral-600 bg-neutral-700 px-3 py-1 text-xs text-white hover:bg-neutral-600"
>
	Flip
</button>

{#if pushMode}
	<p class="mt-3 text-xs text-neutral-400">
		Press-and-release runs <code class="rounded bg-neutral-900 px-1"
			>equibop --toggle-mic</code
		> twice — make sure the displayed state matches reality.
	</p>
{/if}
