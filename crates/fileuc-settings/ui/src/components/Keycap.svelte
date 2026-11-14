<script lang="ts">
  let { label }: { label: string } = $props();
</script>

<kbd class="inline-flex items-center justify-center px-2 py-0.5 rounded text-[11px] font-mono font-medium border border-dark-border bg-dark-card text-gray-200 shadow-sm">
  {label}
</kbd>
