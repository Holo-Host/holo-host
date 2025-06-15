<script lang="ts">
  import { defaultTheme } from "@/theme";

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
  let viewDate = $state(value);
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
  }

  function onViewDateUpdate(month: number) {
    viewDate.setMonth(month, 1);
    viewDate = new Date(viewDate.getTime());
  }
</script>

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
              style:--selected-text-color={defaultTheme.colors.text.white}
              style:--selected-background-color={defaultTheme.colors.background
                .primary}
            >
              {day.day}
            </button>
          </td>
        {/each}
      </tr>
    {/each}
  </tbody>
</table>

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
</style>
