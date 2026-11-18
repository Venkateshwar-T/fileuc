import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type Tab = 'home' | 'plugins' | 'settings';
export type ThemeChoice = 'Dark' | 'Light' | 'System';
export type ModifierChoice = 'Ctrl' | 'Alt' | 'Shift';

export interface PendingUpdate {
  version: string;
  release_url: string;
  release_name: string | null;
  download_url: string | null;
}

export interface UpdateCheckResponse {
  status: 'update_available' | 'up_to_date' | 'offline' | 'error';
  current_version: string;
  update: PendingUpdate | null;
  message: string | null;
}

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
  pending_update?: PendingUpdate | null;
  detailed_progress?: boolean;
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
    detailed_progress: false,
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

  let pendingUpdate = $state<PendingUpdate | null>(null);
  let bannerDismissed = $state<boolean>(false);
  let isCheckingUpdate = $state<boolean>(false);
  let updateStatusMessage = $state<string>('');
  let currentAppVersion = $state<string>('0.1.3');
  let isUpdating = $state<boolean>(false);
  let updateProgress = $state<number>(0);
  let updateProgressText = $state<string>('');
  let updateError = $state<string | null>(null);

  async function applyTheme(theme: ThemeChoice) {
    document.documentElement.classList.add('disable-transitions');

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

    // Force layout reflow so theme color variables update synchronously
    void document.documentElement.offsetHeight;
    requestAnimationFrame(() => {
      document.documentElement.classList.remove('disable-transitions');
    });
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
        if (data.pending_update) {
          pendingUpdate = data.pending_update;
          bannerDismissed = false;
        }
      }
    } catch {
      await applyTheme('System');
    }

    // Automatically check for updates on launch (force = false)
    // If pendingUpdate was already set from disk, backend will instantly return it without hitting GitHub.
    // If none was set, backend will check GitHub (offline resilient).
    await checkForUpdates(false);
  }

  async function checkForUpdates(force = false) {
    if (isCheckingUpdate) return;
    isCheckingUpdate = true;
    updateStatusMessage = '';
    try {
      const res = await invoke<UpdateCheckResponse>('check_for_updates', { force });
      if (res) {
        currentAppVersion = res.current_version;
        if (res.status === 'update_available' && res.update) {
          pendingUpdate = res.update;
          bannerDismissed = false; // Show banner if an update is detected
          updateStatusMessage = `v${res.update.version} available`;
        } else if (res.status === 'up_to_date') {
          pendingUpdate = null;
          if (force) {
            updateStatusMessage = 'Up to date';
            setTimeout(() => {
              if (updateStatusMessage === 'Up to date') updateStatusMessage = '';
            }, 3500);
          }
        } else if (res.status === 'offline') {
          if (force) {
            updateStatusMessage = 'Offline';
            setTimeout(() => {
              if (updateStatusMessage === 'Offline') updateStatusMessage = '';
            }, 3500);
          }
        } else if (force) {
          updateStatusMessage = 'Check failed';
          setTimeout(() => {
            if (updateStatusMessage === 'Check failed') updateStatusMessage = '';
          }, 3500);
        }
      }
    } catch (e) {
      console.warn('Update check failed:', e);
      if (force) {
        updateStatusMessage = 'Offline';
        setTimeout(() => {
          if (updateStatusMessage === 'Offline') updateStatusMessage = '';
        }, 3500);
      }
    } finally {
      isCheckingUpdate = false;
    }
  }

  function dismissBanner() {
    bannerDismissed = true;
  }

  async function openReleaseUrl(url?: string) {
    const targetUrl = url || pendingUpdate?.download_url || pendingUpdate?.release_url;
    if (!targetUrl) return;
    try {
      await invoke('open_release_url', { url: targetUrl });
    } catch (e) {
      console.warn('Failed to open release URL:', e);
    }
  }

  async function startUpdate() {
    if (!pendingUpdate || !pendingUpdate.download_url) {
      await openReleaseUrl();
      return;
    }
    isUpdating = true;
    updateError = null;
    updateProgress = 0.05;
    updateProgressText = 'Connecting to update server...';

    try {
      await invoke('install_update_now', { downloadUrl: pendingUpdate.download_url });
    } catch (e) {
      isUpdating = false;
      updateError = String(e);
      await openReleaseUrl();
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

      await listen<{ progress: number; text: string; done: boolean; error?: string }>('update-progress', (event) => {
        const payload = event.payload;
        updateProgress = payload.progress;
        updateProgressText = payload.text;
        if (payload.error) {
          updateError = payload.error;
          isUpdating = false;
        } else if (payload.done) {
          updateProgress = 1.0;
        }
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
    get pendingUpdate() { return pendingUpdate; },
    get bannerDismissed() { return bannerDismissed; },
    get isCheckingUpdate() { return isCheckingUpdate; },
    get updateStatusMessage() { return updateStatusMessage; },
    get currentAppVersion() { return currentAppVersion; },
    get isUpdating() { return isUpdating; },
    get updateProgress() { return updateProgress; },
    get updateProgressText() { return updateProgressText; },
    get updateError() { return updateError; },
    loadSettings,
    saveSettings,
    toggleService,
    loadPluginStatus,
    installPlugin,
    uninstallPlugin,
    initListeners,
    applyTheme,
    checkForUpdates,
    dismissBanner,
    openReleaseUrl,
    startUpdate,
  };
}

export const store = createSettingsStore();
