<script lang="ts">
  let {
    options,
    value = $bindable(),
    onchange,
  }: {
    options: { label: string; value: string }[];
    value: string;
    onchange?: (val: string) => void;
  } = $props();

  function select(val: string) {
    value = val;
    onchange?.(val);
  }
</script>

<div class="inline-flex rounded-lg bg-dark-control p-1 border border-dark-border">
  {#each options as opt}
    <button
      type="button"
      onclick={() => select(opt.value)}
      class="px-3 py-1 text-xs font-medium rounded-md transition-all duration-150"
      class:bg-accent={value === opt.value}
      class:text-white={value === opt.value}
      class:shadow-sm={value === opt.value}
      class:text-gray-400={value !== opt.value}
      class:hover:text-gray-200={value !== opt.value}
    >
      {opt.label}
    </button>
  {/each}
</div>
