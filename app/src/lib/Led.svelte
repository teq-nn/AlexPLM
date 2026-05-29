<script lang="ts">
  // Small status LED. Status is read-derived elsewhere; in this read-only slice
  // every Baustein shows a static grey "working/at-rest" dot.
  type Status = "free" | "working" | "attention" | "off";

  let { status = "working", title = "" }: { status?: Status; title?: string } =
    $props();

  const colors: Record<Status, string> = {
    free: "var(--led-free)",
    working: "var(--led-working)",
    attention: "var(--led-attention)",
    off: "var(--led-off)",
  };
</script>

<span
  class="led"
  class:attention={status === "attention"}
  style:--c={colors[status]}
  {title}
  aria-label={title || status}
></span>

<style>
  .led {
    --c: var(--led-working);
    display: inline-block;
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--c);
    /* faint seated ring + tiny top highlight so it reads as a physical LED */
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--c) 55%, #000 18%),
      inset 0 1px 0.5px rgba(255, 255, 255, 0.55);
    flex: none;
  }
  /* Only the loud-exception LED glows. Not reached in this slice. */
  .led.attention {
    box-shadow:
      0 0 0 1px color-mix(in srgb, var(--c) 70%, #000 10%),
      0 0 7px 1px color-mix(in srgb, var(--c) 75%, transparent),
      inset 0 1px 0.5px rgba(255, 255, 255, 0.55);
  }
</style>
