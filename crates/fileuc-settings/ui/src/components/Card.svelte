<script lang="ts">
  import type { Snippet } from 'svelte';
  let { children, class: extraClass = '' }: { children: Snippet; class?: string } = $props();
</script>

<div class="rounded-xl border border-dark-border bg-dark-card p-4 shadow-sm {extraClass}">
  {@render children()}
</div>
