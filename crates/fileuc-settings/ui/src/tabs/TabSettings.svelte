<script lang="ts">
  import { store, type ModifierChoice, type ThemeChoice } from '../stores/settings.svelte';
  import Card from '../components/Card.svelte';
  import Segment from '../components/Segment.svelte';
  import Switch from '../components/Switch.svelte';
  import ShortcutRecorder from '../components/ShortcutRecorder.svelte';

  const modOptions: { label: string; value: ModifierChoice }[] = [
    { label: 'Ctrl', value: 'Ctrl' },
    { label: 'Alt', value: 'Alt' },
    { label: 'Shift', value: 'Shift' },
  ];

  const themeOptions: { label: string; value: ThemeChoice }[] = [
    { label: 'System', value: 'System' },
    { label: 'Dark', value: 'Dark' },
    { label: 'Light', value: 'Light' },
  ];
</script>

<div class="space-y-6">
  <!-- Header Title -->
  <div>
    <h1 class="text-xl font-bold tracking-tight text-gray-100 font-display">Preferences</h1>
    <p class="text-xs text-gray-400 mt-1">
      Configure trigger shortcuts, appearance, and background behaviors
    </p>
  </div>

  <!-- Section 1: Shortcuts -->
  <div>
    <div class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 mb-2.5">
      Shortcuts
    </div>

    <Card class="space-y-4">
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm font-semibold text-gray-100">Trigger Modifier</div>
          <div class="text-xs text-gray-400 mt-0.5 leading-normal">
            Hold this key while dragging files to reveal HUD
          </div>
        </div>

        <Segment
          options={modOptions}
          value={store.settings.trigger_modifier}
          onchange={(val) => {
            store.settings.trigger_modifier = val as ModifierChoice;
            store.saveSettings();
          }}
        />
      </div>

      <div class="h-px bg-dark-border w-full"></div>

      <div>
        <div class="text-sm font-semibold text-gray-100">Quick QR Code</div>
        <div class="text-xs text-gray-400 mt-0.5 leading-normal">
          Summon instant QR generator to share links, passwords, or text
        </div>

        <div class="mt-2.5">
          <ShortcutRecorder
            value={store.settings.qr_shortcut}
            defaultValue="Ctrl+Shift+Q"
            onchange={(val) => {
              store.settings.qr_shortcut = val;
              store.saveSettings();
            }}
          />
        </div>
      </div>

      <div class="h-px bg-dark-border w-full"></div>

      <div>
        <div class="text-sm font-semibold text-gray-100">Search Box HUD</div>
        <div class="text-xs text-gray-400 mt-0.5 leading-normal">
          Summon floating search bar and quick action chips for selected files or folders
        </div>

        <div class="mt-2.5">
          <ShortcutRecorder
            value={store.settings.search_shortcut}
            defaultValue="Ctrl+Space"
            onchange={(val) => {
              store.settings.search_shortcut = val;
              store.saveSettings();
            }}
          />
        </div>
      </div>

      <div class="h-px bg-dark-border w-full"></div>

      <div>
        <div class="text-sm font-semibold text-gray-100">Receive from Phone</div>
        <div class="text-xs text-gray-400 mt-0.5 leading-normal">
          Optional shortcut to instantly summon the Receive QR code from anywhere
        </div>

        <div class="mt-2.5">
          <ShortcutRecorder
            value={store.settings.receive_shortcut ?? ''}
            defaultValue=""
            allowEmpty={true}
            onchange={(val) => {
              store.settings.receive_shortcut = val;
              store.saveSettings();
            }}
          />
        </div>
      </div>
    </Card>
  </div>

  <!-- Section 2: Appearance & Startup -->
  <div>
    <div class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 mb-2.5">
      Appearance &amp; Startup
    </div>

    <Card class="space-y-4">
      <!-- Theme Setting -->
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm font-semibold text-gray-100">Theme</div>
          <div class="text-xs text-gray-400 mt-0.5 leading-normal">
            Color scheme for HUD and settings window
          </div>
        </div>

        <Segment
          options={themeOptions}
          value={store.settings.theme}
          onchange={(val) => {
            store.settings.theme = val as ThemeChoice;
            store.applyTheme(val as ThemeChoice);
            store.saveSettings();
          }}
        />
      </div>

      <div class="h-px bg-dark-border w-full"></div>

      <!-- Detailed Progress Indicator Setting -->
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm font-semibold text-gray-100">Detailed Progress Indicator</div>
          <div class="text-xs text-gray-400 mt-0.5 leading-normal">
            Show live estimated completion time and stop button in the floating HUD progress indicator
          </div>
        </div>

        <Switch
          checked={store.settings.detailed_progress ?? false}
          onchange={(val) => {
            store.settings.detailed_progress = val;
            store.saveSettings();
          }}
        />
      </div>

      <div class="h-px bg-dark-border w-full"></div>

      <!-- Auto-Start Setting -->
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm font-semibold text-gray-100">Run on Windows Startup</div>
          <div class="text-xs text-gray-400 mt-0.5 leading-normal">
            Automatically start FileUC in the background on login
          </div>
        </div>

        <Switch
          checked={store.settings.auto_start}
          onchange={(val) => {
            store.settings.auto_start = val;
            store.saveSettings();
          }}
        />
      </div>

      <div class="h-px bg-dark-border w-full"></div>

      <!-- System Tray Setting -->
      <div class="flex items-center justify-between">
        <div>
          <div class="text-sm font-semibold text-gray-100">Show in System Tray</div>
          <div class="text-xs text-gray-400 mt-0.5 leading-normal">
            Display tray icon (runs invisibly in background when off)
          </div>
        </div>

        <Switch
          checked={store.settings.show_tray_icon}
          onchange={(val) => {
            store.settings.show_tray_icon = val;
            store.saveSettings();
          }}
        />
      </div>

      <div class="h-px bg-dark-border w-full"></div>

      <!-- Default Receive Folder Setting -->
      <div class="space-y-2.5">
        <div>
          <div class="text-sm font-semibold text-gray-100">Received Files Location</div>
          <div class="text-xs text-gray-400 mt-0.5 leading-normal">
            Folder where files uploaded from your phone are saved
          </div>
        </div>

        <div class="flex items-center gap-2 flex-wrap">
          <div
            class="flex-1 min-w-0 px-2.5 py-1.5 text-xs text-gray-200 bg-dark-card border border-dark-border rounded-md truncate"
            title={store.settings.receive_folder ?? '%USERPROFILE%\\Documents\\FileUC Received'}
          >
            {store.settings.receive_folder ?? '%USERPROFILE%\\Documents\\FileUC Received'}
          </div>

          <button
            type="button"
            onclick={() => store.pickReceiveFolder()}
            class="px-3 py-1.5 text-xs font-semibold text-white bg-accent hover:bg-accent-hover rounded-md shadow-sm transition-all duration-150 cursor-pointer shrink-0"
          >
            Change Folder
          </button>

          {#if store.settings.receive_folder}
            <button
              type="button"
              onclick={() => {
                store.settings.receive_folder = null;
                store.saveSettings();
              }}
              class="px-2.5 py-1.5 text-xs font-medium text-gray-400 hover:text-gray-200 bg-dark-card hover:bg-dark-control border border-dark-border rounded-md transition-colors cursor-pointer shrink-0"
            >
              Reset to Default
            </button>
          {/if}
        </div>
      </div>
    </Card>
  </div>
</div>
