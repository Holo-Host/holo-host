<script lang="ts">
  import { defaultTheme } from "@/theme";
  import { autoPlacement, computePosition } from "@floating-ui/dom";

  type Prop = {
    value?: Date;
  };

  type day = {
    year: number;
    month: number;
    day: number;
    color: string;
  };
  const days = ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"];

  let { value = $bindable(new Date()) }: Prop = $props();
  let isDropdownOpen = $state(false);
  let viewDate = $state(value);
  const daysTillExpiry = $derived.by(() => {
    return Math.floor((value.getTime() - new Date().getTime()) / 86400000);
  });
  const weeks = $derived.by<day[][]>(() => {
    const startDate = new Date(viewDate.getFullYear(), viewDate.getMonth(), 1);
    const lastDate = new Date(
      viewDate.getFullYear(),
      viewDate.getMonth() + 1,
      0
    );

    let weeks: day[][] = [];
    let days: day[] = [];
    const current = new Date(
      startDate.getFullYear(),
      startDate.getMonth(),
      startDate.getDate() - startDate.getDay()
    );

    while (current < lastDate) {
      for (let i = 0; i < 7; i++) {
        const isCurrentMonth = current.getMonth() == viewDate.getMonth();
        days.push({
          year: current.getFullYear(),
          month: current.getMonth(),
          day: current.getDate(),
          color: isCurrentMonth
            ? defaultTheme.colors.text.black
            : defaultTheme.colors.text.disabled,
        });
        current.setDate(current.getDate() + 1);
      }
      weeks.push(days);
      days = [];
    }

    return weeks;
  });

  function onDateSelected(date: number) {
    value = new Date(viewDate.getFullYear(), viewDate.getMonth(), date);
    isDropdownOpen = false;
  }

  function onViewDateUpdate(month: number) {
    viewDate.setMonth(month, 1);
    viewDate = new Date(viewDate.getTime());
  }

  let datePickerToggleEl: HTMLElement = $state(null);
  let datePickerEl: HTMLElement = $state(null);
  $effect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (!isDropdownOpen) return;
      const target = event.target as HTMLElement;
      if (
        datePickerToggleEl &&
        datePickerEl &&
        (datePickerToggleEl.contains(target) || datePickerEl.contains(target))
      ) {
        return;
      }
      isDropdownOpen = false;
    };

    addEventListener("click", handleClickOutside);

    return () => {
      removeEventListener("click", handleClickOutside);
    };
  });

  $effect(() => {
    if (datePickerToggleEl && datePickerEl) {
      computePosition(datePickerToggleEl, datePickerEl, {
        middleware: [
          autoPlacement({
            allowedPlacements: ["bottom", "top"],
          }),
        ],
      }).then(({ x, y }) => {
        Object.assign(datePickerEl.style, {
          left: `${x}px`,
          top: `${y}px`,
        });
      });
    }
  });
</script>

<button
  class="date-picker-toggle justify-center cursor-pointer"
  style:--border-color={defaultTheme.colors.border}
  onclick={() => (isDropdownOpen = !isDropdownOpen)}
  bind:this={datePickerToggleEl}
>
  <div class="flex gap10 align-center">
    Expires in {daysTillExpiry} days
    <span class="icons-filled">arrow_drop_down</span>
  </div>
</button>
{#if isDropdownOpen}
  <div class="date-picker-container" bind:this={datePickerEl}>
    <div class="date-picker" style:--border-color={defaultTheme.colors.border}>
      <table>
        <thead>
          <tr>
            <td>
              <button
                class="cursor-pointer"
                onclick={() => onViewDateUpdate(viewDate.getMonth() - 1)}
              >
                <span class="icons-filled">arrow_left</span>
              </button>
            </td>
            <td colspan="5" style:text-align="center">
              {viewDate.toLocaleString(navigator.language, {
                month: "short",
                year: "numeric",
              })}
            </td>
            <td>
              <button
                style:width="100%"
                style:align-items="end"
                class="cursor-pointer"
                onclick={() => onViewDateUpdate(viewDate.getMonth() + 1)}
              >
                <span class="icons-filled">arrow_right</span>
              </button>
            </td>
          </tr>
          <tr>
            {#each days as day}
              <th>{day}</th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#each weeks as days}
            <tr>
              {#each days as day}
                <td style:text-align="center" style:color={day.color}>
                  <button
                    onclick={() => onDateSelected(day.day)}
                    class="day"
                    class:selected={value.getDate() === day.day &&
                      value.getMonth() === day.month &&
                      value.getFullYear() === day.year}
                    style:--selected-text-color={defaultTheme.colors.text.black}
                    style:--selected-background-color={defaultTheme.colors
                      .background.secondary}
                  >
                    {day.day}
                  </button>
                </td>
              {/each}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  </div>
{/if}

<style lang="css">
  .day {
    width: 100%;
    cursor: pointer;

    &.selected {
      border-radius: 20px;
      background-color: var(--selected-background-color);
      color: var(--selected-text-color);
    }
  }

  .date-picker-toggle {
    border: 1px solid var(--border-color);
    height: 45px;
    padding-left: 20px;
    padding-right: 10px;
  }

  .date-picker-container {
    display: flex;
    position: absolute;
    width: min-content;
    top: 0;
    left: 0;
    /* top: 20px;
    left: -200px; */
    /* width: 0;
    height: 0; */

    .date-picker {
      display: flex;
      width: 230px;
      height: 200px;
      padding: 10px;
      top: 0;
      z-index: 20;
      background-color: white;
      border: 1px solid var(--border-color);
      border-radius: 7px;
      box-shadow: 8px 8px 10.7px 0px rgba(3, 6, 42, 0.19);
    }
  }
</style>
