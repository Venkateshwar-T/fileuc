<script lang="ts">
  import { onMount } from 'svelte';
  import { store } from './stores/settings.svelte';
  import Sidebar from './components/Sidebar.svelte';
  import TabHome from './tabs/TabHome.svelte';
  import TabPlugins from './tabs/TabPlugins.svelte';
  import TabSettings from './tabs/TabSettings.svelte';

  onMount(async () => {
    await store.loadSettings();
    await store.loadPluginStatus();
    await store.initListeners();
  });
</script>

<div class="flex h-screen w-screen overflow-hidden bg-dark-bg text-gray-200 antialiased select-none font-sans">
  <!-- Left Fixed Sidebar -->
  <Sidebar />

  <!-- Main Scrollable Content Area -->
  <main class="flex-1 h-full overflow-y-auto px-6 py-6">
    <div class="max-w-2xl mx-auto pb-6">
      {#if store.activeTab === 'home'}
        <TabHome />
      {:else if store.activeTab === 'plugins'}
        <TabPlugins />
      {:else if store.activeTab === 'settings'}
        <TabSettings />
      {/if}
    </div>
  </main>
</div>
