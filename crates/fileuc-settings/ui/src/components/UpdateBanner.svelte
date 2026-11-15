<script lang="ts">
  import { Sparkles, ArrowDownToLine, X } from 'lucide-svelte';
  import { store } from '../stores/settings.svelte';

  let isVisible = $derived(Boolean(store.pendingUpdate && !store.bannerDismissed));
</script>

{#if isVisible && store.pendingUpdate}
  <div
    class="relative mb-5 p-3.5 rounded-xl border border-indigo-500/30 bg-gradient-to-r from-indigo-950/40 via-dark-card to-dark-card shadow-lg flex items-center justify-between gap-4 transition-all duration-200"
  >
    <div class="flex items-center gap-3 min-w-0">
      <div class="w-8 h-8 rounded-lg bg-indigo-500/20 text-indigo-400 flex items-center justify-center shrink-0">
        <Sparkles size={18} strokeWidth={2.2} />
      </div>
      <div class="min-w-0">
        <div class="flex items-center gap-2">
          <span class="text-xs font-semibold text-gray-100 tracking-wide">
            Update Available: FileUC v{store.pendingUpdate.version}
          </span>
          {#if store.pendingUpdate.release_name && store.pendingUpdate.release_name !== `FileUC v${store.pendingUpdate.version}`}
            <span class="text-[11px] text-gray-400 truncate hidden sm:inline">
              ({store.pendingUpdate.release_name})
            </span>
          {/if}
        </div>
        <p class="text-[11px] text-gray-400 mt-0.5 truncate">
          A newer version is ready. Click Download to install the latest release.
        </p>
      </div>
    </div>

    <div class="flex items-center gap-2 shrink-0">
      <button
        type="button"
        onclick={() => store.openReleaseUrl()}
        class="inline-flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium bg-[#7066E0] hover:bg-[#5E52D6] text-white shadow transition-all duration-150 cursor-pointer active:scale-95"
      >
        <ArrowDownToLine size={13} strokeWidth={2.2} />
        <span>Download</span>
      </button>

      <button
        type="button"
        onclick={() => store.dismissBanner()}
        aria-label="Dismiss banner"
        class="p-1.5 rounded-lg text-gray-400 hover:text-gray-200 hover:bg-white/10 transition-colors cursor-pointer"
        title="Dismiss for this session"
      >
        <X size={15} strokeWidth={2} />
      </button>
    </div>
  </div>
{/if}
