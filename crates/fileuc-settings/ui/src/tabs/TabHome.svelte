<script lang="ts">
  import { store } from '../stores/settings.svelte';
  import Keycap from '../components/Keycap.svelte';
  import Card from '../components/Card.svelte';
  import { Image, FileText, FileArchive, Film, Archive, Smartphone } from 'lucide-svelte';
</script>

<div class="space-y-6">
  <!-- Header Title & Shortcut -->
  <div class="flex items-start justify-between">
    <div>
      <h1 class="text-xl font-bold tracking-tight text-gray-100 font-display">FileUC</h1>
      <p class="text-xs text-gray-400 mt-1">
        Universal instant file converter and local share engine
      </p>
    </div>

    <!-- Clean Simple Trigger Indicator -->
    <div class="flex items-center gap-1.5 text-xs text-gray-400 mt-0.5">
      <span>Trigger:</span>
      <Keycap label={store.settings.trigger_modifier} />
      <span class="text-gray-400 font-medium">+</span>
      <Keycap label="Drag" />
    </div>
  </div>

  <!-- 3 Steps Grid -->
  <div>
    <div class="text-[11px] font-semibold uppercase tracking-wider text-gray-400 mb-2.5">
      Quick Start
    </div>
    <div class="grid grid-cols-3 gap-3">
      <!-- Step 1 -->
      <Card class="flex flex-col justify-between">
        <div>
          <div class="flex items-center gap-2">
            <span class="inline-flex items-center justify-center h-5 w-5 rounded-full bg-accent text-[11px] font-bold text-white shadow-sm">
              1
            </span>
            <span class="text-sm font-semibold text-gray-100">Select Files</span>
          </div>
          <p class="text-xs text-gray-400 mt-1 leading-normal">
            Pick any file, image, document, video, or audio in Windows Explorer.
          </p>
        </div>
      </Card>

      <!-- Step 2 -->
      <Card class="flex flex-col justify-between">
        <div>
          <div class="flex items-center gap-2">
            <span class="inline-flex items-center justify-center h-5 w-5 rounded-full bg-accent text-[11px] font-bold text-white shadow-sm">
              2
            </span>
            <span class="text-sm font-semibold text-gray-100">Hold &amp; Drag</span>
          </div>
          <p class="text-xs text-gray-400 mt-1 leading-normal">
            Hold <strong class="text-gray-200 font-semibold">{store.settings.trigger_modifier}</strong> while right-click dragging to summon the Donut HUD ring.
          </p>
        </div>
      </Card>

      <!-- Step 3 -->
      <Card class="flex flex-col justify-between">
        <div>
          <div class="flex items-center gap-2">
            <span class="inline-flex items-center justify-center h-5 w-5 rounded-full bg-accent text-[11px] font-bold text-white shadow-sm">
              3
            </span>
            <span class="text-sm font-semibold text-gray-100">Drop to Convert</span>
          </div>
          <p class="text-xs text-gray-400 mt-1 leading-normal">
            Release over any action slice to convert or share in milliseconds.
          </p>
        </div>
      </Card>
    </div>
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

      <!-- Feature 6: Local Share -->
      <Card>
        <div class="flex items-start gap-3">
          <div class="p-2 rounded-lg shrink-0 text-accent" style="background-color: rgba(112, 102, 224, 0.12);">
            <Smartphone size={16} />
          </div>
          <div>
            <div class="text-sm font-semibold text-gray-100">Local Wi-Fi Mobile Share</div>
            <div class="text-xs text-gray-400 mt-1 leading-normal">
              Stream files to phone via QR scan, or press <strong class="text-gray-200 font-semibold">{store.settings.qr_shortcut}</strong> for Instant QR generator.
            </div>
          </div>
        </div>
      </Card>
    </div>
  </div>
</div>
