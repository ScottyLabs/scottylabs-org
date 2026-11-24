// The current time for a page that stays open a while, such as the composer: its presets and "tomorrow" or "6 days from now" follow the clock past midnight. Create it while a component initialises; it stops with the component.

const MINUTE = 60_000;

export class Clock {
  #now = $state(new Date());

  constructor(intervalMs: number = MINUTE) {
    $effect(() => {
      const timer = setInterval(() => {
        this.#now = new Date();
      }, intervalMs);
      return () => {
        clearInterval(timer);
      };
    });
  }

  get now(): Date {
    return this.#now;
  }
}
