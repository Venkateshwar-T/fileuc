<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { store } from './stores/settings.svelte';
  import { UPDATE_CONFIG } from './config/updateConfig';
  import Sidebar from './components/Sidebar.svelte';
  import UpdateBanner from './components/UpdateBanner.svelte';
  import ActiveConversionBar, { type ActiveConversionInfo } from './components/ActiveConversionBar.svelte';
  import TabHome from './tabs/TabHome.svelte';
  import TabPlugins from './tabs/TabPlugins.svelte';
  import TabSettings from './tabs/TabSettings.svelte';

  let activeConversion = $state<ActiveConversionInfo | null>(null);
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  let hideAppShellForBlocker = $derived(
    store.isBlockingUpdate && UPDATE_CONFIG.blockingUiStyle === 'fullpage',
  );

  onMount(async () => {
    await store.loadSettings();
    await store.loadPluginStatus();
    await store.initListeners();

    // Zero-overhead live conversion polling across Home, Plugins, and Settings tabs
    pollInterval = setInterval(async () => {
      try {
        const info = await invoke<ActiveConversionInfo | null>('get_active_conversion');
        if (info && info.is_active) {
          activeConversion = info;
        } else {
          activeConversion = null;
        }
      } catch {
        activeConversion = null;
      }
    }, 150);
  });

  onDestroy(() => {
    if (pollInterval) {
      clearInterval(pollInterval);
      pollInterval = null;
    }
  });
</script>

<div class="flex h-screen w-screen overflow-hidden bg-dark-bg text-gray-200 antialiased select-none font-sans">
  {#if !store.isInitialized}
    <!-- Empty themed frame while persisted settings resolve (~10ms) to prevent UI flash -->
  {:else if hideAppShellForBlocker}
    <UpdateBanner />
  {:else}
    <!-- Left Fixed Sidebar -->
    <Sidebar />

    <!-- Main Scrollable Content Area -->
    <main class="flex-1 h-full overflow-y-auto px-6 py-6">
      <div class="max-w-2xl mx-auto pb-6 space-y-6">
        <UpdateBanner />

        {#if store.activeTab === 'home'}
          <TabHome />
        {:else if store.activeTab === 'plugins'}
          <TabPlugins />
        {:else if store.activeTab === 'settings'}
          <TabSettings />
        {/if}

        {#if activeConversion && activeConversion.is_active}
          <ActiveConversionBar info={activeConversion} />
        {/if}
      </div>
    </main>
  {/if}
</div>
