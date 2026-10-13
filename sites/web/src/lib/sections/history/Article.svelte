<script lang="ts">
  import { ExternalLink } from "@lucide/svelte";
  import type { ArticleBlock } from "#lib/data/history.ts";

  let { article }: { article: ArticleBlock } = $props();

  const display = $derived(article.title ?? article.url);
</script>

<a
  href={article.url}
  target="_blank"
  rel="noopener noreferrer"
  style="display: inline-flex; align-items: center; gap: 8px; color: #819AFF; text-decoration: none; font-family: Satoshi, sans-serif; font-size: 14px; word-break: break-all"
>
  <ExternalLink size={14} style="flex-shrink: 0" />
  <span>{display}</span>
</a>
