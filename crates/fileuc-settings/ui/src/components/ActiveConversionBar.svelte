<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { ArrowRight, Square, Loader2 } from 'lucide-svelte';

  export interface ActiveConversionInfo {
    is_active: boolean;
    file_summary: string;
    action_display: string;
    progress: number;
    eta_text: string;
  }

  let { info } = $props<{ info: ActiveConversionInfo }>();

  let isStopping = $state(false);

  async function handleStop() {
    if (isStopping) return;
    isStopping = true;
    try {
      await invoke('cancel_active_conversion');
    } catch (e) {
      console.error('Failed to cancel conversion:', e);
    }
  }
</script>

<div
  class="sticky bottom-0 z-30 w-full rounded-xl p-4 shadow-xl border border-dark-border bg-dark-card/95 backdrop-blur-md transition-all duration-300 animate-in fade-in slide-in-from-bottom-2"
>
  <div class="flex items-center justify-between gap-4">
    <!-- Left: Files + Arrow + Action Badge -->
    <div class="flex items-center gap-2.5 min-w-0">
      <div class="w-2.5 h-2.5 rounded-full bg-emerald-500 animate-pulse shrink-0"></div>
      <span class="text-sm font-semibold text-gray-900 dark:text-gray-100 truncate" title={info.file_summary}>
        {info.file_summary}
      </span>
      <div class="flex items-center gap-1.5 shrink-0 px-2.5 py-0.5 rounded-md text-xs font-bold bg-accent/15 text-accent border border-accent/20">
        <ArrowRight size={13} class="stroke-[2.5]" />
        <span>{info.action_display}</span>
      </div>
    </div>

    <!-- Right: ETA + Stop Button -->
    <div class="flex items-center gap-3 shrink-0">
      <span class="text-xs font-medium text-gray-500 dark:text-gray-400">
        {info.eta_text}
      </span>
      <button
        type="button"
        onclick={handleStop}
        disabled={isStopping}
        class="flex items-center gap-1.5 px-3 py-1 rounded-md text-xs font-semibold bg-red-500/10 text-red-600 dark:text-red-400 hover:bg-red-500/20 hover:text-red-700 dark:hover:text-red-300 active:scale-95 transition-all border border-red-500/20 disabled:opacity-50 cursor-pointer"
        title="Stop operation"
      >
        {#if isStopping}
          <Loader2 size={11} class="animate-spin" />
          <span>Stopping...</span>
        {:else}
          <Square size={10} fill="currentColor" />
          <span>Stop</span>
        {/if}
      </button>
    </div>
  </div>

  <!-- Bottom: Horizontal Progress Bar -->
  <div class="mt-3 flex items-center gap-3">
    <div class="flex-1 h-2 rounded-full overflow-hidden bg-black/10 dark:bg-white/10">
      <div
        class="h-full rounded-full transition-all duration-150 ease-out"
        style="width: {Math.max(2, Math.min(100, Math.round(info.progress * 100)))}%; background: linear-gradient(90deg, #10B981 0%, #2ED573 100%);"
      ></div>
    </div>
    <span class="text-xs font-bold text-emerald-600 dark:text-emerald-400 w-10 text-right">
      {Math.round(info.progress * 100)}%
    </span>
  </div>
</div>
