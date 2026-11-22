<script lang="ts">
  let {
    value = 'Ctrl+Shift+Q',
    defaultValue = 'Ctrl+Shift+Q',
    onchange,
  }: {
    value: string;
    defaultValue?: string;
    onchange: (val: string) => void;
  } = $props();

  let isRecording = $state(false);
  let recordedMods = $state<string[]>([]);
  let warningMsg = $state<string | null>(null);

  let keyBadges = $derived(value ? value.split('+').map((s) => s.trim()) : []);
  let isCustom = $derived(value !== defaultValue);

  function startRecording() {
    isRecording = true;
    recordedMods = [];
    warningMsg = null;
    window.addEventListener('keydown', handleKeyDown, true);
    window.addEventListener('keyup', handleKeyUp, true);
    window.addEventListener('blur', stopRecording);
  }

  function stopRecording() {
    isRecording = false;
    recordedMods = [];
    warningMsg = null;
    window.removeEventListener('keydown', handleKeyDown, true);
    window.removeEventListener('keyup', handleKeyUp, true);
    window.removeEventListener('blur', stopRecording);
  }

  // Ensure listeners are cleaned up if component unmounts mid-recording
  $effect(() => {
    return () => {
      if (isRecording) {
        stopRecording();
      }
    };
  });

  function handleKeyDown(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();

    if (e.key === 'Escape') {
      stopRecording();
      return;
    }

    const isModifierKey = ['Control', 'Shift', 'Alt', 'Meta'].includes(e.key);

    const mods: string[] = [];
    if (e.ctrlKey) mods.push('Ctrl');
    if (e.altKey) mods.push('Alt');
    if (e.shiftKey) mods.push('Shift');

    if (isModifierKey) {
      recordedMods = mods;
      warningMsg = null;
      return;
    }

    const isFKey = e.code.startsWith('F') && /^F\d+$/.test(e.code);

    // Safety guard: Single alphanumeric keys without Ctrl/Alt/Shift are rejected
    // to prevent swallowing normal text typing across Windows.
    if (mods.length === 0 && !isFKey) {
      warningMsg = 'Include Ctrl, Alt, or Shift';
      return;
    }

    // A valid key combination was pressed: finalize combo!
    let keyName = e.key.toUpperCase();
    if (e.code.startsWith('Key')) {
      keyName = e.code.substring(3).toUpperCase();
    } else if (e.code.startsWith('Digit')) {
      keyName = e.code.substring(5);
    } else if (isFKey) {
      keyName = e.code;
    } else if (e.key === ' ') {
      keyName = 'Space';
    } else if (e.key === 'Enter') {
      keyName = 'Enter';
    } else if (e.key === 'Tab') {
      keyName = 'Tab';
    }

    const finalCombo = [...mods, keyName].join('+');
    stopRecording();
    onchange(finalCombo);
  }

  function handleKeyUp(e: KeyboardEvent) {
    e.preventDefault();
    e.stopPropagation();
  }

  function resetToDefault() {
    onchange(defaultValue);
  }
</script>

<div class="flex items-center gap-3 flex-wrap">
  {#if isRecording}
    <div
      class="flex items-center gap-2 px-3 py-1.5 rounded-md border border-accent bg-accent/15 text-xs font-medium text-accent animate-pulse"
    >
      <span class="w-2 h-2 rounded-full bg-accent animate-ping"></span>
      <span>
        {#if warningMsg}
          <span class="text-amber-300 font-medium">{warningMsg}</span>
        {:else if recordedMods.length > 0}
          {recordedMods.join(' + ')} + ...
        {:else}
          Press desired keys (or Esc to cancel)...
        {/if}
      </span>
      <button
        type="button"
        onclick={stopRecording}
        class="ml-2 text-gray-400 hover:text-gray-200 text-xs underline cursor-pointer"
      >
        Cancel
      </button>
    </div>
  {:else}
    <!-- Current Trigger Display (clean standalone badges, no redundant outer box) -->
    <div class="flex items-center gap-1.5" title="Current shortcut trigger">
      {#each keyBadges as k, idx}
        <kbd
          class="px-2 py-0.5 text-[11.5px] font-sans font-medium text-gray-200 bg-dark-card border border-dark-border rounded shadow-sm"
        >
          {k}
        </kbd>
        {#if idx < keyBadges.length - 1}
          <span class="text-[11px] text-gray-500 font-bold">+</span>
        {/if}
      {/each}
    </div>

    <!-- Configure Button (matched to accent color of switches and active tabs) -->
    <button
      type="button"
      onclick={startRecording}
      title="Click to configure new shortcut"
      class="px-3 py-1 text-xs font-semibold text-white bg-accent hover:bg-accent-hover rounded-md shadow-sm transition-all duration-150 cursor-pointer"
    >
      Configure
    </button>

    <!-- Restore to Default Button -->
    {#if isCustom}
      <button
        type="button"
        onclick={resetToDefault}
        title="Restore default shortcut ({defaultValue})"
        class="px-2.5 py-1 text-xs font-medium text-gray-400 hover:text-gray-200 bg-dark-card hover:bg-dark-control border border-dark-border rounded-md transition-colors cursor-pointer"
      >
        Restore Default
      </button>
    {/if}
  {/if}
</div>
