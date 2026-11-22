<script lang="ts">
  import { store } from '../stores/settings.svelte';
  import Keycap from '../components/Keycap.svelte';
  import Card from '../components/Card.svelte';
  import { Image, FileText, FileArchive, Film, Archive, Smartphone } from 'lucide-svelte';

  let activeHudTab = $state<'donut' | 'search'>('donut');
  const isDark = $derived(store.isDarkMode);

  interface DonutSlice {
    label: string;
    active?: boolean;
  }

  const donutSlices: DonutSlice[] = [
    { label: 'WebP' },
    { label: 'PNG', active: true },
    { label: 'JPG' },
    { label: 'SVG' },
    { label: 'Icon' },
    { label: 'Share' },
    { label: 'Reduce' },
  ];

  function getSlicePath(index: number, total: number, active: boolean) {
    const cx = 170;
    const cy = 170;
    const base_outer_r = 145;
    const base_inner_r = 62;
    const corner_r = 9;
    const gap_px = 3.2;

    const span = (2 * Math.PI) / total;
    const midAngle = -Math.PI / 2 + index * span;

    const outer_half = Math.max(0.01, span / 2 - gap_px / (2 * base_outer_r));
    const inner_half = Math.max(0.01, span / 2 - gap_px / (2 * base_inner_r));

    const d_angle_out = corner_r / base_outer_r;
    const d_angle_in = corner_r / base_inner_r;
    const d_r = corner_r;

    const pop = active ? 7 : 0;
    const scx = cx + Math.cos(midAngle) * pop;
    const scy = cy + Math.sin(midAngle) * pop;

    const pt = (angle: number, radius: number) => ({
      x: scx + Math.cos(angle) * radius,
      y: scy + Math.sin(angle) * radius,
    });

    const start_angle_outer = midAngle - outer_half;
    const end_angle_outer = midAngle + outer_half;
    const start_angle_inner = midAngle - inner_half;
    const end_angle_inner = midAngle + inner_half;

    const p_start = pt(start_angle_outer + d_angle_out, base_outer_r);
    const p_out_end = pt(end_angle_outer - d_angle_out, base_outer_r);
    const c_out_end = pt(end_angle_outer, base_outer_r);
    const p_line_top = pt(end_angle_outer, base_outer_r - d_r);
    const p_line_bot = pt(end_angle_inner, base_inner_r + d_r);
    const c_in_end = pt(end_angle_inner, base_inner_r);
    const p_in_end = pt(end_angle_inner - d_angle_in, base_inner_r);
    const p_in_start = pt(start_angle_inner + d_angle_in, base_inner_r);
    const c_in_start = pt(start_angle_inner, base_inner_r);
    const p_start_bot = pt(start_angle_inner, base_inner_r + d_r);
    const p_start_top = pt(start_angle_outer, base_outer_r - d_r);
    const c_out_start = pt(start_angle_outer, base_outer_r);

    return `M ${p_start.x} ${p_start.y}
      A ${base_outer_r} ${base_outer_r} 0 0 1 ${p_out_end.x} ${p_out_end.y}
      Q ${c_out_end.x} ${c_out_end.y} ${p_line_top.x} ${p_line_top.y}
      L ${p_line_bot.x} ${p_line_bot.y}
      Q ${c_in_end.x} ${c_in_end.y} ${p_in_end.x} ${p_in_end.y}
      A ${base_inner_r} ${base_inner_r} 0 0 0 ${p_in_start.x} ${p_in_start.y}
      Q ${c_in_start.x} ${c_in_start.y} ${p_start_bot.x} ${p_start_bot.y}
      L ${p_start_top.x} ${p_start_top.y}
      Q ${c_out_start.x} ${c_out_start.y} ${p_start.x} ${p_start.y}
      Z`;
  }

  function getTextPos(index: number, total: number, active: boolean) {
    const cx = 170;
    const cy = 170;
    const span = (2 * Math.PI) / total;
    const midAngle = -Math.PI / 2 + index * span;
    const pop = active ? 7 : 0;
    const scx = cx + Math.cos(midAngle) * pop;
    const scy = cy + Math.sin(midAngle) * pop;
    const textR = (145 + 62) / 2; // 103.5

    return {
      x: scx + Math.cos(midAngle) * textR,
      y: scy + Math.sin(midAngle) * textR + 4.5,
    };
  }
</script>

<div class="space-y-6">
  <!-- Header Title -->
  <div>
    <h1 class="text-xl font-bold tracking-tight text-gray-100 font-display">FileUC</h1>
    <p class="text-xs text-gray-400 mt-1">
      Instant desktop file utility, quick actions, and mobile sharing engine
    </p>
  </div>

  <!-- Interactive HUD Showcase with Tab Switcher -->
  <div>
    <div class="flex items-center justify-between mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-wider text-gray-400">
        Trigger Modes
      </div>
      <div class="inline-flex rounded-lg bg-dark-control p-1 border border-dark-border">
        <button
          type="button"
          onclick={() => (activeHudTab = 'donut')}
          class="px-3.5 py-1 text-xs font-medium rounded-md transition-all duration-150 cursor-pointer"
          class:bg-accent={activeHudTab === 'donut'}
          class:text-white={activeHudTab === 'donut'}
          class:shadow-sm={activeHudTab === 'donut'}
          class:text-gray-400={activeHudTab !== 'donut'}
          class:hover:text-gray-200={activeHudTab !== 'donut'}
        >
          Donut HUD
        </button>
        <button
          type="button"
          onclick={() => (activeHudTab = 'search')}
          class="px-3.5 py-1 text-xs font-medium rounded-md transition-all duration-150 cursor-pointer"
          class:bg-accent={activeHudTab === 'search'}
          class:text-white={activeHudTab === 'search'}
          class:shadow-sm={activeHudTab === 'search'}
          class:text-gray-400={activeHudTab !== 'search'}
          class:hover:text-gray-200={activeHudTab !== 'search'}
        >
          Search HUD
        </button>
      </div>
    </div>

    <Card class="p-5 flex flex-col justify-between">
      {#if activeHudTab === 'donut'}
        <!-- Donut Tab -->
        <div>
          <div class="flex items-center justify-between border-b border-dark-border/60 pb-3 mb-4">
            <div class="flex items-center gap-2 text-xs text-gray-300">
              <span class="text-gray-400 font-medium">Shortcut:</span>
              <Keycap label={`${store.settings.trigger_modifier} + Drag`} />
              <span class="text-gray-500 text-[11px] hidden sm:inline">
                (Hold modifier while dragging over files in Windows Explorer)
              </span>
            </div>
          </div>

          <div class="flex flex-col items-center justify-center py-2">
            <svg viewBox="0 0 340 340" class="w-64 h-64 select-none overflow-visible">
              <defs>
                <filter id="active-glow" x="-25%" y="-25%" width="150%" height="150%">
                  <feDropShadow dx="0" dy="2" stdDeviation="4" flood-color={isDark ? '#7066E0' : '#6C63D8'} flood-opacity="0.45" />
                </filter>
                <filter id="donut-shadow" x="-30%" y="-30%" width="160%" height="160%">
                  <feGaussianBlur in="SourceAlpha" stdDeviation="6" />
                  <feOffset dx="0" dy="4" result="offsetblur" />
                  <feFlood flood-color="#000000" flood-opacity={isDark ? '0.30' : '0.10'} />
                  <feComposite in2="offsetblur" operator="in" />
                </filter>
                <filter id="hub-shadow" x="-25%" y="-25%" width="150%" height="150%">
                  <feDropShadow dx="0" dy="2" stdDeviation="4" flood-color="#000000" flood-opacity={isDark ? '0.45' : '0.12'} />
                </filter>
              </defs>

              <!-- Layer 1: Bottom Ambient Donut Shadow (prevents slice shadows from rendering over adjacent slices) -->
              <g filter="url(#donut-shadow)" class="pointer-events-none">
                {#each donutSlices as slice, i}
                  {@const path = getSlicePath(i, donutSlices.length, !!slice.active)}
                  <path d={path} fill="#000000" />
                {/each}
              </g>

              <!-- Layer 2: Inactive Radial Slices -->
              {#each donutSlices as slice, i}
                {#if !slice.active}
                  {@const path = getSlicePath(i, donutSlices.length, false)}
                  <path
                    d={path}
                    fill={isDark ? '#242426' : '#FFFFFF'}
                    stroke={isDark ? 'rgba(255, 255, 255, 0.086)' : 'rgba(0, 0, 0, 0.08)'}
                    stroke-width="1"
                    class="transition-colors duration-150"
                  />
                {/if}
              {/each}

              <!-- Layer 3: Active / Popped Slice (rendered above inactive slices with accent glow) -->
              {#each donutSlices as slice, i}
                {#if slice.active}
                  {@const path = getSlicePath(i, donutSlices.length, true)}
                  <path
                    d={path}
                    fill={isDark ? '#7066E0' : '#6C63D8'}
                    stroke={isDark ? '#A39CF4' : '#4B41B9'}
                    stroke-width="1.5"
                    filter="url(#active-glow)"
                    class="transition-colors duration-150"
                  />
                {/if}
              {/each}

              <!-- Layer 4: Typography Labels -->
              {#each donutSlices as slice, i}
                {@const isHover = slice.active}
                {@const textPos = getTextPos(i, donutSlices.length, !!isHover)}
                <text
                  x={textPos.x}
                  y={textPos.y}
                  text-anchor="middle"
                  font-size="14"
                  font-weight={isHover ? '700' : '600'}
                  fill={isHover ? '#FFFFFF' : (isDark ? '#F5F3EE' : '#1E1D1C')}
                  class="select-none font-sans"
                >
                  {slice.label}
                </text>
              {/each}

              <!-- Center Hub -->
              <circle
                cx="170"
                cy="170"
                r="56"
                fill={isDark ? '#1A1A1C' : '#FCFCFC'}
                stroke={isDark ? 'rgba(255, 255, 255, 0.086)' : 'rgba(0, 0, 0, 0.08)'}
                stroke-width="1.2"
                filter="url(#hub-shadow)"
              />
              <text
                x="170"
                y="164"
                text-anchor="middle"
                font-size="14.5"
                font-weight="700"
                fill={isDark ? '#F5F3EE' : '#1E1D1C'}
                class="select-none font-display tracking-tight"
              >
                1 File
              </text>
              <text
                x="170"
                y="181"
                text-anchor="middle"
                font-size="11"
                font-weight="500"
                fill={isDark ? '#AFADA6' : '#6E6C68'}
                class="select-none font-sans"
              >
                Convert to PNG
              </text>
            </svg>

            <p class="text-xs text-gray-400 mt-2 text-center max-w-sm leading-normal">
              Hover over any slice and release mouse button to execute instant conversion or action.
            </p>
          </div>
        </div>
      {:else}
        <!-- Search Tab -->
        <div>
          <div class="flex items-center justify-between border-b border-dark-border/60 pb-3 mb-4">
            <div class="flex items-center gap-2 text-xs text-gray-300">
              <span class="text-gray-400 font-medium">Shortcut:</span>
              <Keycap label={store.settings.search_shortcut.replace(/\+/g, ' + ')} />
              <span class="text-gray-500 text-[11px] hidden sm:inline">
                (Press shortcut with any file selected or from clipboard)
              </span>
            </div>
          </div>

          <div class="flex flex-col items-center justify-center py-6">
            <!-- Simulated High-DPI Search Pill -->
            <div
              class="w-full max-w-sm h-11 rounded-full flex items-center px-2 relative transition-all"
              style="background-color: {isDark ? '#1C1C1E' : '#FFFFFF'}; border: 1px solid {isDark ? 'rgba(255, 255, 255, 0.086)' : 'rgba(0, 0, 0, 0.08)'}; box-shadow: {isDark ? '0 4px 14px rgba(0, 0, 0, 0.28)' : '0 4px 14px rgba(0, 0, 0, 0.08)'};"
            >
              <img
                src="/search-icon.png"
                alt="Search"
                class="w-7 h-7 rounded-full ml-1 mr-2.5 shrink-0 shadow-sm object-cover"
              />
              <div class="flex items-center flex-1 overflow-hidden">
                <span class="w-[1.5px] h-4.5 animate-pulse mr-2 shrink-0" style="background-color: {isDark ? '#F5F3EE' : '#1E1D1C'};"></span>
                <span class="text-[13.5px] font-normal tracking-tight select-none" style="color: {isDark ? '#8E8D88' : '#9E9C96'};">Convert to PNG</span>
              </div>
            </div>

            <!-- Floating Action Chips Matching Actual HUD -->
            <div class="flex flex-wrap items-center justify-center gap-2 mt-4 max-w-sm">
              <div
                class="font-semibold px-4 py-1.5 rounded-full text-xs flex items-center gap-1.5 cursor-default transition-all"
                style="background-color: {isDark ? '#7066E0' : '#6C63D8'}; border: 1px solid {isDark ? '#A39CF4' : '#4B41B9'}; color: #FFFFFF; box-shadow: {isDark ? '0 4px 14px rgba(0, 0, 0, 0.28)' : '0 4px 14px rgba(0, 0, 0, 0.08)'};"
              >
                <span>PNG</span>
              </div>
              <div
                class="px-3.5 py-1.5 rounded-full text-xs cursor-default transition-all"
                style="background-color: {isDark ? '#242426' : '#FFFFFF'}; border: 1px solid {isDark ? 'rgba(255, 255, 255, 0.086)' : 'rgba(0, 0, 0, 0.08)'}; color: {isDark ? '#F5F3EE' : '#1E1D1C'}; box-shadow: {isDark ? '0 4px 14px rgba(0, 0, 0, 0.28)' : '0 4px 14px rgba(0, 0, 0, 0.08)'};"
              >
                <span>WebP</span>
              </div>
              <div
                class="px-3.5 py-1.5 rounded-full text-xs cursor-default transition-all"
                style="background-color: {isDark ? '#242426' : '#FFFFFF'}; border: 1px solid {isDark ? 'rgba(255, 255, 255, 0.086)' : 'rgba(0, 0, 0, 0.08)'}; color: {isDark ? '#F5F3EE' : '#1E1D1C'}; box-shadow: {isDark ? '0 4px 14px rgba(0, 0, 0, 0.28)' : '0 4px 14px rgba(0, 0, 0, 0.08)'};"
              >
                <span>JPG</span>
              </div>
              <div
                class="px-3.5 py-1.5 rounded-full text-xs cursor-default transition-all"
                style="background-color: {isDark ? '#242426' : '#FFFFFF'}; border: 1px solid {isDark ? 'rgba(255, 255, 255, 0.086)' : 'rgba(0, 0, 0, 0.08)'}; color: {isDark ? '#F5F3EE' : '#1E1D1C'}; box-shadow: {isDark ? '0 4px 14px rgba(0, 0, 0, 0.28)' : '0 4px 14px rgba(0, 0, 0, 0.08)'};"
              >
                <span>SVG</span>
              </div>
              <div
                class="px-3.5 py-1.5 rounded-full text-xs cursor-default transition-all"
                style="background-color: {isDark ? '#242426' : '#FFFFFF'}; border: 1px solid {isDark ? 'rgba(255, 255, 255, 0.086)' : 'rgba(0, 0, 0, 0.08)'}; color: {isDark ? '#F5F3EE' : '#1E1D1C'}; box-shadow: {isDark ? '0 4px 14px rgba(0, 0, 0, 0.28)' : '0 4px 14px rgba(0, 0, 0, 0.08)'};"
              >
                <span>Share</span>
              </div>
            </div>

            <p class="text-xs text-gray-400 mt-5 text-center max-w-sm leading-normal">
              Type natural keywords or action names and press Enter to instantly run with 0 latency.
            </p>
          </div>
        </div>
      {/if}
    </Card>
  </div>

  <!-- Features & Capabilities -->
  <div>
    <div class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 mb-2.5">
      Capabilities &amp; Features
    </div>
    <div class="grid grid-cols-2 gap-3">
      <!-- Feature 1: Images -->
      <Card>
        <div class="flex items-start gap-3">
          <div class="p-2 rounded-lg shrink-0 text-accent" style="background-color: rgba(112, 102, 224, 0.12);">
            <Image size={16} />
          </div>
          <div>
            <div class="text-sm font-semibold text-gray-100">Image Format Conversion</div>
            <div class="text-xs text-gray-400 mt-1 leading-normal">
              Convert between PNG, JPG, WebP, SVG, BMP, ICO, and TIFF formats.
            </div>
          </div>
        </div>
      </Card>

      <!-- Feature 2: Documents -->
      <Card>
        <div class="flex items-start gap-3">
          <div class="p-2 rounded-lg shrink-0 text-accent" style="background-color: rgba(112, 102, 224, 0.12);">
            <FileText size={16} />
          </div>
          <div>
            <div class="text-sm font-semibold text-gray-100">Office &amp; Document Engine</div>
            <div class="text-xs text-gray-400 mt-1 leading-normal">
              PDF to DOCX Word, Excel, PPTX to PDF, Markdown &amp; TXT document exports.
            </div>
          </div>
        </div>
      </Card>

      <!-- Feature 3: PDF Toolkit -->
      <Card>
        <div class="flex items-start gap-3">
          <div class="p-2 rounded-lg shrink-0 text-accent" style="background-color: rgba(112, 102, 224, 0.12);">
            <FileArchive size={16} />
          </div>
          <div>
            <div class="text-sm font-semibold text-gray-100">PDF Power Tools</div>
            <div class="text-xs text-gray-400 mt-1 leading-normal">
              Merge, split, compress, lock/unlock with password &amp; OCR text recognition.
            </div>
          </div>
        </div>
      </Card>

      <!-- Feature 4: Media & Audio -->
      <Card>
        <div class="flex items-start gap-3">
          <div class="p-2 rounded-lg shrink-0 text-accent" style="background-color: rgba(112, 102, 224, 0.12);">
            <Film size={16} />
          </div>
          <div>
            <div class="text-sm font-semibold text-gray-100">Video &amp; Audio Processing</div>
            <div class="text-xs text-gray-400 mt-1 leading-normal">
              Convert video to GIF, extract audio to MP3, and compress high-bitrate video.
            </div>
          </div>
        </div>
      </Card>

      <!-- Feature 5: Archives -->
      <Card>
        <div class="flex items-start gap-3">
          <div class="p-2 rounded-lg shrink-0 text-accent" style="background-color: rgba(112, 102, 224, 0.12);">
            <Archive size={16} />
          </div>
          <div>
            <div class="text-sm font-semibold text-gray-100">Archives &amp; Bundling</div>
            <div class="text-xs text-gray-400 mt-1 leading-normal">
              Create instant ZIP, 7Z, multi-file One Zip bundles, and single-click unpack.
            </div>
          </div>
        </div>
      </Card>

      <!-- Feature 6: File Share -->
      <Card>
        <div class="flex items-start gap-3">
          <div class="p-2 rounded-lg shrink-0 text-accent" style="background-color: rgba(112, 102, 224, 0.12);">
            <Smartphone size={16} />
          </div>
          <div>
            <div class="text-sm font-semibold text-gray-100">File Share</div>
            <div class="text-xs text-gray-400 mt-1 leading-normal">
              Stream and share files via QR scan, or press <strong class="text-gray-200 font-semibold">{store.settings.qr_shortcut}</strong> for Instant QR generator.
            </div>
          </div>
        </div>
      </Card>
    </div>
  </div>
</div>
