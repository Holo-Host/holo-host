<script lang="ts">
  const diamonds = $derived.by(() => {
    const colors = ["#0017C1", "#00B7FF"];
    const amount = 10;
    const pattern: string[] = [];
    for (let i = 0; i < amount; i++) {
      const colorIndex = i % colors.length;
      pattern.push(colors[colorIndex]);
    }
    return pattern;
  });
</script>

<div class="pattern-wrapper">
  <div class="diamond-grid">
    {#each diamonds as color}
      <div class="diamond" style:background-color={color}></div>
    {/each}
  </div>
</div>

<style>
  .pattern-wrapper {
    position: fixed;
    bottom: 0;
    right: 0;
    z-index: 0;
    width: 100vw;
    height: 100vh;
    pointer-events: none;
    overflow: visible;
  }

  .diamond-grid {
    position: absolute;
    bottom: calc(20vw * 0.3);
    right: calc(20vw * -2.5);
    display: grid;
    gap: 50px;
    grid-template-columns: repeat(4, 20vw);
    grid-template-rows: repeat(2, 20vw);
    transform: scaleY(0.6) rotate(-45deg);
    transform-origin: bottom right;
  }

  .diamond {
    width: 20vw;
    height: 20vw;
  }
</style>
