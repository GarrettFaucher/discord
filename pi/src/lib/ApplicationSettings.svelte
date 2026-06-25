<script lang="ts">
	import { globalSettings, openUrl } from "@openaction/svelte-pi";

	const DEFAULT_PORT = 6789;
	const PLUGIN_REPO = "https://github.com/GarrettFaucher/equibop-opendeck";

	function updatePort(event: Event) {
		const parsed = parseInt((event.target as HTMLInputElement).value, 10);
		const port =
			Number.isFinite(parsed) && parsed > 0 && parsed < 65536
				? parsed
				: DEFAULT_PORT;
		$globalSettings = { ...$globalSettings, port };
	}
</script>

<h2 class="mb-3 text-sm font-semibold text-neutral-100">Equibop Bridge</h2>

{#if $globalSettings.error}
	<div
		class="mb-3 rounded-lg border border-red-700 bg-red-900/30 p-2 text-xs text-red-300"
	>
		<strong class="font-semibold">Error:</strong>
		{$globalSettings.error}
	</div>
{/if}

<div class="mb-3 flex items-center gap-2">
	<span class="min-w-22.5 text-xs font-medium text-neutral-200">
		Bridge Port:
	</span>
	<input
		id="port"
		type="number"
		min="1"
		max="65535"
		value={$globalSettings.port ?? DEFAULT_PORT}
		onchange={updatePort}
		class="w-24 rounded-lg border border-neutral-600 bg-neutral-700 px-2 py-1 text-xs text-neutral-100 focus:border-neutral-600 focus:ring-1 focus:ring-neutral-600 focus:outline-none"
	/>
</div>

<div class="rounded-lg border border-neutral-600 bg-neutral-700 p-3 text-xs">
	<p class="mb-2 font-medium text-neutral-200">
		This plugin controls Discord through Equibop.
	</p>
	<ol class="ml-1 list-inside list-decimal space-y-1.5 text-neutral-300">
		<li>
			Install the
			<button
				onclick={() => openUrl(PLUGIN_REPO)}
				class="cursor-pointer text-blue-400 underline hover:text-blue-300"
			>
				EquibopOpenDeck
			</button>
			userplugin in Equibop (build from source — see its README).
		</li>
		<li>
			Make sure its bridge port matches the <strong>Bridge Port</strong> above
			(default {DEFAULT_PORT}).
		</li>
		<li>Restart Equibop and enable the plugin in Settings → Plugins.</li>
	</ol>
</div>
