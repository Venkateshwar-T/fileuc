<script lang="ts">
  import { store } from '../stores/settings.svelte';
  import Card from '../components/Card.svelte';
  import { Loader2 } from 'lucide-svelte';
</script>

<div class="space-y-6">
  <!-- Header Title -->
  <div>
    <h1 class="text-xl font-bold tracking-tight text-gray-100 font-display">Plugins &amp; Engines</h1>
    <p class="text-xs text-gray-400 mt-1">
      Optional offline processing engines for specialized media and document conversions
    </p>
  </div>

  <!-- Plugins List -->
  <div>
    <div class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 mb-2.5">
      Offline Engines
    </div>

    <div class="space-y-3">
      <!-- FFmpeg Plugin Card -->
      <Card class="space-y-3.5">
        <div>
          <h3 class="text-sm font-semibold text-gray-100">FFmpeg Media Engine</h3>
          <p class="text-xs text-gray-400 mt-1 leading-normal">
            Required for converting and optimizing audio/video files (MP4, MP3, GIF, WebM, WAV).
          </p>
        </div>

        <!-- Bottom Status & Action Row -->
        {#if store.ffmpeg.downloading}
          <div class="flex items-center justify-between gap-4">
            <div class="flex items-center gap-2 text-xs text-accent">
              <Loader2 size={14} class="animate-spin shrink-0" />
              <span class="truncate">Installing... {store.ffmpeg.statusText}</span>
            </div>

            <div class="flex items-center gap-3 shrink-0">
              <div class="w-32 h-1.5 rounded-full bg-dark-control overflow-hidden border border-dark-border">
                <div
                  class="h-full bg-accent transition-all duration-200"
                  style="width: {Math.round(store.ffmpeg.progress * 100)}%"
                ></div>
              </div>
              <span class="text-xs font-mono text-accent">{Math.round(store.ffmpeg.progress * 100)}%</span>
            </div>
          </div>
        {:else}
          <div class="flex items-center justify-between">
            <!-- Status on Left -->
            <div class="text-xs font-medium">
              {#if store.ffmpeg.installed}
                <span class="text-green-500 font-medium">Installed</span>
              {:else}
                <span class="text-red-500 font-medium">Not installed</span>
              {/if}
            </div>

            <!-- Action on Right -->
            <div>
              {#if store.ffmpeg.installed}
                <button
                  type="button"
                  onclick={() => store.uninstallPlugin('ffmpeg')}
                  class="px-3.5 py-1.5 text-xs font-medium rounded-lg border border-red-500/25 bg-red-500/10 hover:bg-red-500/20 hover:border-red-500/40 text-red-400 hover:text-red-300 transition-all duration-150 cursor-pointer shadow-sm active:scale-[0.98]"
                >
                  Uninstall
                </button>
              {:else}
                <button
                  type="button"
                  onclick={() => store.installPlugin('ffmpeg')}
                  class="px-4 py-1.5 text-xs font-medium text-white rounded-lg shadow-sm bg-accent hover:bg-accent-hover transition-all duration-150 cursor-pointer active:scale-[0.98]"
                >
                  Install
                </button>
              {/if}
            </div>
          </div>
        {/if}
      </Card>

      <!-- DOCX Plugin Card -->
      <Card class="space-y-3.5">
        <div>
          <h3 class="text-sm font-semibold text-gray-100">DOCX Document Layout Engine</h3>
          <p class="text-xs text-gray-400 mt-1 leading-normal">
            Enables accurate PDF to Word DOCX extraction with preserved table grids and layout.
          </p>
        </div>

        <!-- Bottom Status & Action Row -->
        {#if store.docx.downloading}
          <div class="flex items-center justify-between gap-4">
            <div class="flex items-center gap-2 text-xs text-accent">
              <Loader2 size={14} class="animate-spin shrink-0" />
              <span class="truncate">Installing... {store.docx.statusText}</span>
            </div>

            <div class="flex items-center gap-3 shrink-0">
              <div class="w-32 h-1.5 rounded-full bg-dark-control overflow-hidden border border-dark-border">
                <div
                  class="h-full bg-accent transition-all duration-200"
                  style="width: {Math.round(store.docx.progress * 100)}%"
                ></div>
              </div>
              <span class="text-xs font-mono text-accent">{Math.round(store.docx.progress * 100)}%</span>
            </div>
          </div>
        {:else}
          <div class="flex items-center justify-between">
            <!-- Status on Left -->
            <div class="text-xs font-medium">
              {#if store.docx.installed}
                <span class="text-green-500 font-medium">Installed</span>
              {:else}
                <span class="text-red-500 font-medium">Not installed</span>
              {/if}
            </div>

            <!-- Action on Right -->
            <div>
              {#if store.docx.installed}
                <button
                  type="button"
                  onclick={() => store.uninstallPlugin('docx')}
                  class="px-3.5 py-1.5 text-xs font-medium rounded-lg border border-red-500/25 bg-red-500/10 hover:bg-red-500/20 hover:border-red-500/40 text-red-400 hover:text-red-300 transition-all duration-150 cursor-pointer shadow-sm active:scale-[0.98]"
                >
                  Uninstall
                </button>
              {:else}
                <button
                  type="button"
                  onclick={() => store.installPlugin('docx')}
                  class="px-4 py-1.5 text-xs font-medium text-white rounded-lg shadow-sm bg-accent hover:bg-accent-hover transition-all duration-150 cursor-pointer active:scale-[0.98]"
                >
                  Install
                </button>
              {/if}
            </div>
          </div>
        {/if}
      </Card>
    </div>
  </div>
</div>
