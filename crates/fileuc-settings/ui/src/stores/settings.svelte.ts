import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type Tab = 'home' | 'plugins' | 'settings';
export type ThemeChoice = 'Dark' | 'Light' | 'System';
export type ModifierChoice = 'Ctrl' | 'Alt' | 'Shift';

export interface AppSettings {
  trigger_modifier: string;
  qr_shortcut: string;
  theme: ThemeChoice;
  accent_color: string;
  ffmpeg_path: string | null;
  auto_start: boolean;
  show_tray_icon: boolean;
  service_enabled: boolean;
  hud_style: string;
  output_mode: string;
}

export interface PluginState {
  installed: boolean;
  downloading: boolean;
  progress: number;
  statusText: string;
}

export function createSettingsStore() {
  let activeTab = $state<Tab>('home');
  let isDarkMode = $state<boolean>(true);
  let listenersInitialized = false;

  let settings = $state<AppSettings>({
    trigger_modifier: 'Ctrl',
    qr_shortcut: 'Ctrl+Shift+Q',
    theme: 'Dark',
    accent_color: '#7066E0',
    ffmpeg_path: null,
    auto_start: true,
    show_tray_icon: true,
    service_enabled: true,
    hud_style: 'donut',
    output_mode: 'ask',
  });

  let ffmpeg = $state<PluginState>({
    installed: false,
    downloading: false,
    progress: 0,
    statusText: '',
  });

  let docx = $state<PluginState>({
    installed: false,
    downloading: false,
    progress: 0,
    statusText: '',
  });

  let uninstalledCount = $derived((ffmpeg.installed ? 0 : 1) + (docx.installed ? 0 : 1));

  async function applyTheme(theme: ThemeChoice) {
    if (theme === 'Light') {
      isDarkMode = false;
      document.documentElement.classList.remove('dark');
      document.documentElement.classList.add('light');
    } else if (theme === 'Dark') {
      isDarkMode = true;
      document.documentElement.classList.remove('light');
      document.documentElement.classList.add('dark');
    } else {
      let isDark = false;
      try {
        const sysTheme = await invoke<string>('get_system_theme');
        isDark = sysTheme === 'Dark';
      } catch {
        isDark = Boolean(window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches);
      }
      isDarkMode = isDark;
      if (isDark) {
        document.documentElement.classList.remove('light');
        document.documentElement.classList.add('dark');
      } else {
        document.documentElement.classList.remove('dark');
        document.documentElement.classList.add('light');
      }
    }
  }

  async function loadSettings() {
    try {
      const tabIdx = await invoke<number>('get_initial_tab');
      if (tabIdx === 1) activeTab = 'plugins';
      else if (tabIdx === 2) activeTab = 'settings';
    } catch {}

    try {
      const data = await invoke<AppSettings>('get_settings');
      if (data) {
        settings = data;
        await applyTheme(data.theme);
      }
    } catch {
      await applyTheme('System');
    }
  }

  async function saveSettings() {
    try {
      const payload = $state.snapshot(settings);
      await invoke('save_settings', { newSettings: payload });
    } catch (e) {
      console.warn('save_settings failed:', e);
    }
  }

  async function toggleService(enabled: boolean) {
    settings.service_enabled = enabled;
    try {
      await invoke('toggle_service', { enabled });
    } catch (e) {
      console.warn('toggle_service failed:', e);
    }
  }

  async function loadPluginStatus() {
    try {
      const status = await invoke<{ ffmpeg_installed: boolean; docx_installed: boolean }>('get_plugin_status');
      ffmpeg.installed = status.ffmpeg_installed;
      docx.installed = status.docx_installed;
    } catch {
      // Mock fallback
    }
  }

  async function installPlugin(name: 'ffmpeg' | 'docx') {
    const target = name === 'ffmpeg' ? ffmpeg : docx;
    target.downloading = true;
    target.progress = 0;
    target.statusText = 'Starting...';

    try {
      await invoke('install_plugin', { name });
    } catch (e) {
      target.downloading = false;
      target.statusText = String(e);
    }
  }

  async function uninstallPlugin(name: 'ffmpeg' | 'docx') {
    const target = name === 'ffmpeg' ? ffmpeg : docx;
    try {
      await invoke('uninstall_plugin', { name });
      target.installed = false;
      target.downloading = false;
      target.progress = 0;
      target.statusText = '';
    } catch (e) {
      console.warn('uninstall failed:', e);
    }
  }

  // Setup real-time Tauri event listeners (guarded against duplicate attachment)
  async function initListeners() {
    if (listenersInitialized) return;
    listenersInitialized = true;

    try {
      await listen<{ name: string; progress: number; text: string; done: boolean; error?: string }>('plugin-progress', (event) => {
        const payload = event.payload;
        const target = payload.name === 'ffmpeg' ? ffmpeg : docx;
        target.progress = payload.progress;
        target.statusText = payload.text;

        if (payload.done) {
          target.downloading = false;
          if (!payload.error) {
            target.installed = true;
            target.progress = 1.0;
          }
        }
      });

      await listen<{ enabled: boolean }>('service-toggled', (event) => {
        settings.service_enabled = event.payload.enabled;
      });
    } catch {
      // Not in Tauri context (browser preview)
    }
  }

  return {
    get activeTab() { return activeTab; },
    set activeTab(val: Tab) { activeTab = val; },
    get isDarkMode() { return isDarkMode; },
    get settings() { return settings; },
    get ffmpeg() { return ffmpeg; },
    get docx() { return docx; },
    get uninstalledCount() { return uninstalledCount; },
    loadSettings,
    saveSettings,
    toggleService,
    loadPluginStatus,
    installPlugin,
    uninstallPlugin,
    initListeners,
    applyTheme,
  };
}

export const store = createSettingsStore();
