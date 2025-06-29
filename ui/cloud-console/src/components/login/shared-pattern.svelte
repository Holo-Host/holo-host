<script lang="ts">
  let patternEl: HTMLElement = $state(null);
  $effect(() => {
    for (let i = 0; i < 10; i++) {
      const box = document.createElement("div");
      box.style.width = "200px";
      box.style.height = "200px";
      if (i % 2 === 0) {
        box.style.backgroundColor = "rgba(1, 186, 255, 1)";
      } else {
        box.style.backgroundColor = "rgba(27, 33, 188, 1)";
      }
      patternEl.appendChild(box);
    }
  });
</script>

<div bind:this={patternEl} class="shared-pattern gap20">
</div>

<style lang="css">
  .shared-pattern {
    position: fixed;
    bottom: 200px;
    right: 200px;
  }
</style>
