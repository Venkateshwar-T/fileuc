<script lang="ts">
  import { ArrowDownToLine, Loader2, ShieldAlert, X } from 'lucide-svelte';
  import { store } from '../stores/settings.svelte';
  import { UPDATE_CONFIG } from '../config/updateConfig';

  let pending = $derived(store.pendingUpdate);
  let isBlocking = $derived(store.isBlockingUpdate);
  let isVisible = $derived(Boolean(pending && (isBlocking || !store.bannerDismissed)));

  let standardBodyText = $derived.by(() => {
    if (!pending) return '';
    if (UPDATE_CONFIG.standardCopyMode === 'value') {
      return pending.release_notes || UPDATE_CONFIG.copy.standardValueFallback;
    }
    return UPDATE_CONFIG.copy.standardSimpleBody;
  });

  let downloadingText = $derived(
    pending ? UPDATE_CONFIG.copy.downloadingLabel(pending.version) : 'Downloading...',
  );
</script>

{#snippet downloadingHeader(titleClass: string)}
  <div class="flex items-center justify-between gap-3">
    <span class={titleClass}>{downloadingText}</span>
    <span class="px-2 py-0.5 rounded-md bg-accent/15 border border-accent/25 text-accent text-xs font-mono font-semibold shrink-0">
      {Math.round(store.updateProgress * 100)}%
    </span>
  </div>
{/snippet}

{#snippet progressTrack()}
  <div class="w-full bg-dark-control rounded-full h-2 overflow-hidden border border-dark-border">
    <div
      class="bg-accent h-full rounded-full transition-all duration-150 ease-out"
      style="width: {Math.max(store.updateProgress * 100, 4)}%"
    ></div>
  </div>
{/snippet}

{#if isVisible && pending}
  {#if isBlocking && UPDATE_CONFIG.blockingUiStyle === 'fullpage'}
    <!-- Critical / Forced Update: Unboxed Full-Page Blocker Directly on App Body -->
    <div
      class="fixed inset-0 z-50 bg-dark-bg flex flex-col items-center justify-center p-8 text-center select-none"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="blocking-update-title"
    >
      <div class="w-full max-w-md flex flex-col items-center gap-5">
        <div class="w-12 h-12 rounded-2xl bg-accent text-white shadow-md shadow-accent/25 flex items-center justify-center">
          {#if store.isUpdating}
            <Loader2 size={24} strokeWidth={2.4} class="animate-spin" />
          {:else}
            <ShieldAlert size={24} strokeWidth={2.2} />
          {/if}
        </div>

        {#if store.isUpdating}
          <div id="blocking-update-title" class="w-full max-w-xs flex flex-col gap-3 text-left">
            {@render downloadingHeader('text-sm font-bold text-gray-900 dark:text-gray-100')}
            {@render progressTrack()}
          </div>
        {:else}
          <div class="flex flex-col items-center gap-2">
            <h1 id="blocking-update-title" class="text-xl font-bold text-gray-900 dark:text-gray-100 tracking-tight">
              {UPDATE_CONFIG.copy.blockingHeadline}
            </h1>
            <p class="text-xs text-gray-600 dark:text-gray-300 leading-relaxed max-w-sm">
              {#if store.updateError}
                <span class="text-red-500 font-medium">{store.updateError}</span>
              {:else}
                {UPDATE_CONFIG.copy.blockingBody}
              {/if}
            </p>
          </div>

          <button
            type="button"
            onclick={() => store.startUpdate()}
            class="mt-1 inline-flex items-center justify-center gap-2 px-6 py-2.5 rounded-xl text-xs font-semibold bg-accent hover:bg-accent-hover text-white shadow-md shadow-accent/25 transition-all duration-150 cursor-pointer active:scale-[0.98]"
          >
            <ArrowDownToLine size={14} strokeWidth={2.4} />
            <span>{UPDATE_CONFIG.copy.updateButtonLabel}</span>
          </button>
        {/if}
      </div>
    </div>
  {:else if isBlocking}
    <!-- Critical / Forced Update: Fixed Non-Closable Modal over Blurred App -->
    <div
      class="fixed inset-0 z-50 bg-black/65 backdrop-blur-sm flex items-center justify-center p-6 select-none"
      role="alertdialog"
      aria-modal="true"
      aria-labelledby="blocking-update-title"
    >
      <div class="w-full max-w-md p-6 rounded-2xl border border-dark-border bg-dark-card shadow-2xl flex flex-col gap-5">
        <div class="flex items-start gap-3.5">
          <div class="w-10 h-10 rounded-xl bg-accent text-white shadow-md shadow-accent/25 flex items-center justify-center shrink-0">
            {#if store.isUpdating}
              <Loader2 size={20} strokeWidth={2.4} class="animate-spin" />
            {:else}
              <ShieldAlert size={20} strokeWidth={2.2} />
            {/if}
          </div>

          <div class="min-w-0 flex-1">
            {#if store.isUpdating}
              <div id="blocking-update-title">
                {@render downloadingHeader('text-sm font-bold text-gray-900 dark:text-gray-100')}
              </div>
            {:else}
              <h2 id="blocking-update-title" class="text-base font-bold text-gray-900 dark:text-gray-100">
                {UPDATE_CONFIG.copy.blockingHeadline}
              </h2>
              <p class="text-xs text-gray-600 dark:text-gray-300 mt-1.5 leading-relaxed">
                {#if store.updateError}
                  <span class="text-red-500 font-medium">{store.updateError}</span>
                {:else}
                  {UPDATE_CONFIG.copy.blockingBody}
                {/if}
              </p>
            {/if}
          </div>
        </div>

        {#if store.isUpdating}
          {@render progressTrack()}
        {:else}
          <button
            type="button"
            onclick={() => store.startUpdate()}
            class="w-full inline-flex items-center justify-center gap-2 px-4 py-2.5 rounded-xl text-xs font-semibold bg-accent hover:bg-accent-hover text-white shadow-md shadow-accent/25 transition-all duration-150 cursor-pointer active:scale-[0.99]"
          >
            <ArrowDownToLine size={14} strokeWidth={2.4} />
            <span>{UPDATE_CONFIG.copy.updateButtonLabel}</span>
          </button>
        {/if}
      </div>
    </div>
  {:else if UPDATE_CONFIG.standardUiStyle === 'modal'}
    <!-- Non-blocking Feature Highlights Modal -->
    <div
      class="fixed inset-0 z-50 bg-black/50 backdrop-blur-sm flex items-center justify-center p-6 select-none"
      role="dialog"
      aria-modal="true"
    >
      <div class="w-full max-w-md p-6 rounded-2xl border border-dark-border bg-dark-card shadow-2xl flex flex-col gap-5">
        <div class="flex items-start gap-3.5">
          <div class="w-10 h-10 rounded-xl bg-accent text-white shadow-md shadow-accent/25 flex items-center justify-center shrink-0">
            {#if store.isUpdating}
              <Loader2 size={20} strokeWidth={2.4} class="animate-spin" />
            {:else}
              <ArrowDownToLine size={20} strokeWidth={2.4} />
            {/if}
          </div>

          <div class="min-w-0 flex-1">
            {#if store.isUpdating}
              {@render downloadingHeader('text-sm font-bold text-gray-900 dark:text-gray-100')}
            {:else}
              <h2 class="text-base font-bold text-gray-900 dark:text-gray-100">
                {UPDATE_CONFIG.copy.featureModalHeadline(pending.version)}
              </h2>
              <p class="text-xs text-gray-600 dark:text-gray-300 mt-1.5 leading-relaxed">
                {#if store.updateError}
                  <span class="text-red-500 font-medium">{store.updateError}</span>
                {:else}
                  {pending.release_notes || UPDATE_CONFIG.copy.standardValueFallback}
                {/if}
              </p>
            {/if}
          </div>
        </div>

        {#if store.isUpdating}
          {@render progressTrack()}
        {:else}
          <div class="flex items-center justify-end gap-2.5">
            <button
              type="button"
              onclick={() => store.dismissBanner()}
              class="px-3.5 py-2 rounded-lg text-xs font-medium text-gray-500 dark:text-gray-300 hover:bg-black/5 dark:hover:bg-white/10 transition-colors cursor-pointer"
            >
              {UPDATE_CONFIG.copy.laterButtonLabel}
            </button>
            <button
              type="button"
              onclick={() => store.startUpdate()}
              class="inline-flex items-center gap-1.5 px-4 py-2 rounded-lg text-xs font-semibold bg-accent hover:bg-accent-hover text-white shadow-md shadow-accent/25 transition-all duration-150 cursor-pointer active:scale-95"
            >
              <ArrowDownToLine size={13} strokeWidth={2.4} />
              <span>{UPDATE_CONFIG.copy.updateButtonLabel}</span>
            </button>
          </div>
        {/if}
      </div>
    </div>
  {:else}
    <!-- Standard Non-blocking Top Banner / Toast -->
    <div
      class="{UPDATE_CONFIG.standardUiStyle === 'toast'
        ? 'fixed bottom-5 right-5 z-40 w-96 shadow-2xl'
        : 'relative mb-5 shadow-lg'} p-4 rounded-xl border border-accent/30 bg-dark-card flex flex-col gap-3 transition-all duration-200"
    >
      <div class="flex items-center justify-between gap-4">
        <div class="flex items-center gap-3 min-w-0 flex-1">
          <div
            class="w-9 h-9 rounded-xl bg-accent text-white shadow-md shadow-accent/25 border border-accent/40 flex items-center justify-center shrink-0"
          >
            {#if store.isUpdating}
              <Loader2 size={18} strokeWidth={2.4} class="animate-spin" />
            {:else}
              <ArrowDownToLine size={18} strokeWidth={2.4} />
            {/if}
          </div>

          <div class="min-w-0 flex-1">
            {#if store.isUpdating}
              <span class="text-xs font-bold text-gray-900 dark:text-gray-100 tracking-wide block">
                {downloadingText}
              </span>
            {:else}
              <span class="text-xs font-bold text-gray-900 dark:text-gray-100 tracking-wide block">
                {UPDATE_CONFIG.copy.standardHeadline(pending.version)}
              </span>
              <p class="text-[11px] text-gray-600 dark:text-gray-300 mt-0.5">
                {#if store.updateError}
                  <span class="text-red-500 font-medium">{store.updateError}</span>
                {:else}
                  {standardBodyText}
                {/if}
              </p>
            {/if}
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
            >
              <ArrowDownToLine size={13} strokeWidth={2.4} />
              <span>{UPDATE_CONFIG.copy.updateButtonLabel}</span>
            </button>

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

      {#if store.isUpdating}
        {@render progressTrack()}
      {/if}
    </div>
  {/if}
{/if}
