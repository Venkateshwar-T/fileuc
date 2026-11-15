<script lang="ts">
  import { Home, Blocks, Sliders } from 'lucide-svelte';
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

        {#if item.id === 'plugins' && store.uninstalledCount > 0}
          <span
            class="px-1.5 py-0.5 rounded-full text-[10px] font-semibold bg-opacity-20"
            class:bg-accent={isActive}
            class:text-accent={isActive}
            class:bg-amber-500={!isActive}
            class:text-amber-400={!isActive}
          >
            {store.uninstalledCount}
          </span>
        {/if}
      </button>
    {/each}
  </nav>

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
</aside>
