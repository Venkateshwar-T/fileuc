<script lang="ts">
  import { Home, Blocks, Sliders, RefreshCw, Check } from 'lucide-svelte';
  import { store, type Tab } from '../stores/settings.svelte';
  import Switch from './Switch.svelte';

  const navItems: { id: Tab; label: string; icon: typeof Home }[] = [
    { id: 'home', label: 'Home', icon: Home },
    { id: 'plugins', label: 'Plugins', icon: Blocks },
    { id: 'settings', label: 'Settings', icon: Sliders },
  ];
</script>

<aside class="w-48 h-full bg-dark-sidebar border-r border-dark-border flex flex-col justify-between p-3 select-none">
  <!-- Top Navigation Tabs -->
  <nav class="flex flex-col space-y-1">
    {#each navItems as item}
      {@const isActive = store.activeTab === item.id}
      <button
        type="button"
        onclick={() => (store.activeTab = item.id)}
        class="flex items-center justify-between w-full px-3 py-2 rounded-lg text-xs font-medium transition-all duration-150 {isActive ? 'text-accent' : 'text-gray-400 hover:text-gray-200 hover:bg-white/5'}"
        style={isActive ? 'background-color: rgba(112, 102, 224, 0.15); color: #7066E0;' : ''}
      >
        <div class="flex items-center gap-2.5">
          <item.icon size={16} strokeWidth={2} class="shrink-0" />
          <span class="leading-none">{item.label}</span>
        </div>
      </button>
    {/each}
  </nav>

  <!-- Bottom Section: Update Button & Service Switch -->
  <div class="flex flex-col gap-2 mt-auto">
    <!-- Manual Update Button (Above Active Switch) -->
    <button
      type="button"
      onclick={() => store.checkForUpdates(true)}
      disabled={store.isCheckingUpdate}
      class="flex items-center justify-between w-full px-3 py-2 rounded-lg border border-dark-border bg-dark-card hover:bg-white/5 text-xs font-medium cursor-pointer disabled:opacity-60 disabled:cursor-wait"
      title="Check GitHub for FileUC updates"
    >
      <div class="flex items-center gap-2 truncate">
        <RefreshCw
          size={13}
          strokeWidth={2}
          class="shrink-0 text-gray-400 {store.isCheckingUpdate ? 'animate-spin text-accent' : ''}"
        />
        <span class="truncate text-gray-300">
          {#if store.isCheckingUpdate}
            Checking...
          {:else if store.updateStatusMessage}
            {store.updateStatusMessage}
          {:else if store.pendingUpdate}
            Update Ready
          {:else}
            Check Updates
          {/if}
        </span>
      </div>

      {#if store.pendingUpdate && !store.isCheckingUpdate}
        <span class="h-2 w-2 rounded-full bg-accent animate-pulse shrink-0"></span>
      {:else if store.updateStatusMessage === 'Up to date'}
        <Check size={13} strokeWidth={2.5} class="text-green-400 shrink-0" />
      {/if}
    </button>

    <!-- Bottom Service Status Card -->
    <div class="p-3 rounded-lg border border-dark-border bg-dark-card flex items-center justify-between shadow-sm">
      <div class="flex items-center gap-2">
        <span
          class="h-2 w-2 rounded-full"
          class:bg-green-500={store.settings.service_enabled}
          class:bg-amber-500={!store.settings.service_enabled}
        ></span>
        <span class="text-xs font-medium text-gray-200">
          {store.settings.service_enabled ? 'Active' : 'Inactive'}
        </span>
      </div>

      <Switch
        checked={store.settings.service_enabled}
        onchange={(val) => store.toggleService(val)}
      />
    </div>
  </div>
</aside>
