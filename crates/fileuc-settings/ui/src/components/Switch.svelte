<script lang="ts">
  let { checked = $bindable(false), onchange }: { checked?: boolean; onchange?: (val: boolean) => void } = $props();

  function toggle() {
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  type="button"
  role="switch"
  aria-label="Toggle switch"
  aria-checked={checked}
  onclick={toggle}
  class="relative inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full transition-colors duration-200 focus:outline-none"
  class:bg-accent={checked}
  class:bg-gray-300={!checked}
  class:dark:bg-gray-600={!checked}
>
  <span
    class="pointer-events-none inline-block h-3.5 w-3.5 transform rounded-full bg-white shadow-sm ring-0 transition duration-200 ease-in-out"
    class:translate-x-4={checked}
    class:translate-x-1={!checked}
  ></span>
</button>
