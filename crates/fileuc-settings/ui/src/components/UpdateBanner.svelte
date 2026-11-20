<script lang="ts">
  import { ArrowDownToLine, Loader2, X } from 'lucide-svelte';
  import { store } from '../stores/settings.svelte';

  let isVisible = $derived(Boolean(store.pendingUpdate && !store.bannerDismissed));
</script>

{#if isVisible && store.pendingUpdate}
  <div
    class="relative mb-5 p-4 rounded-xl border border-accent/30 bg-dark-card shadow-lg flex flex-col gap-3 transition-all duration-200"
  >
    <div class="flex items-center justify-between gap-4">
      <div class="flex items-center gap-3 min-w-0">
        <!-- High-contrast Solid Icon Box -->
        <div
          class="w-9 h-9 rounded-xl bg-accent text-white shadow-md shadow-accent/25 border border-accent/40 flex items-center justify-center shrink-0"
        >
          {#if store.isUpdating}
            <Loader2 size={18} strokeWidth={2.4} class="animate-spin" />
          {:else}
            <ArrowDownToLine size={18} strokeWidth={2.4} />
          {/if}
        </div>

        <div class="min-w-0">
          <div class="flex items-center gap-2">
            <span class="text-xs font-bold text-gray-900 dark:text-gray-100 tracking-wide">
              {#if store.isUpdating}
                Updating FileUC to v{store.pendingUpdate.version}...
              {:else}
                Update Available: FileUC v{store.pendingUpdate.version}
              {/if}
            </span>
            {#if store.pendingUpdate.release_name && store.pendingUpdate.release_name !== `FileUC v${store.pendingUpdate.version}`}
              <span class="text-[11px] text-gray-500 dark:text-gray-400 truncate hidden sm:inline">
                ({store.pendingUpdate.release_name})
              </span>
            {/if}
          </div>

          <p class="text-[11px] text-gray-600 dark:text-gray-300 mt-0.5 truncate">
            {#if store.isUpdating}
              {store.updateProgressText || 'Downloading update in background...'}
            {:else if store.updateError}
              <span class="text-red-500 font-medium">Download failed: {store.updateError}.</span>
            {:else}
              A newer version is ready. Click "Update Now" to install and seamlessly restart.
            {/if}
          </p>
        </div>
      </div>

      <div class="flex items-center gap-2 shrink-0">
        {#if store.isUpdating}
          <div class="px-2.5 py-1 rounded-md bg-accent/15 border border-accent/25 text-accent text-xs font-mono font-semibold">
            {Math.round(store.updateProgress * 100)}%
          </div>
        {:else}
          <button
            type="button"
            onclick={() => store.startUpdate()}
            class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white shadow-md shadow-accent/25 border border-accent/30 transition-all duration-150 cursor-pointer active:scale-95"
            title="Download update and seamlessly restart"
          >
            <ArrowDownToLine size={13} strokeWidth={2.4} />
            <span>Update Now</span>
          </button>
        {/if}

        {#if !store.isUpdating}
          <button
            type="button"
            onclick={() => store.dismissBanner()}
            aria-label="Dismiss banner"
            class="p-1.5 rounded-lg text-gray-400 hover:text-gray-600 dark:hover:text-gray-200 hover:bg-black/5 dark:hover:bg-white/10 transition-colors cursor-pointer"
            title="Dismiss for this session"
          >
            <X size={15} strokeWidth={2} />
          </button>
        {/if}
      </div>
    </div>

    <!-- Real-time Progress Bar during in-app update -->
    {#if store.isUpdating}
      <div class="w-full bg-dark-control rounded-full h-2 overflow-hidden border border-dark-border">
        <div
          class="bg-accent h-full rounded-full transition-all duration-150 ease-out"
          style="width: {Math.max(store.updateProgress * 100, 4)}%"
        ></div>
      </div>
    {/if}
  </div>
{/if}
