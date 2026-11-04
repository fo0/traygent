import { readable, type Readable } from 'svelte/store';

export const MINUTE = 60_000;

/** Sets the current time while the clock runs; null while it is stopped. */
let catchUp: (() => void) | null = null;

const clock = readable(Date.now(), (set) => {
  const tick = () => set(Date.now());
  let timer: ReturnType<typeof setInterval> | undefined;
  // Reads now and counts the next minute from here.
  const restart = () => {
    clearInterval(timer);
    tick();
    timer = setInterval(tick, MINUTE);
  };
  catchUp = restart;
  restart();
  const visible = () => {
    if (document.visibilityState === 'visible') tick();
  };
  const watchesVisibility = typeof document !== 'undefined';
  if (watchesVisibility) document.addEventListener('visibilitychange', visible);
  return () => {
    catchUp = null;
    clearInterval(timer);
    if (watchesVisibility)
      document.removeEventListener('visibilitychange', visible);
  };
});

/**
 * Wall-clock time in milliseconds for relative times („vor 3 Min.“).
 * One shared interval serves every subscriber; it starts with the first one
 * and stops with the last, so rows never own timers. A panel that becomes
 * visible again catches up immediately instead of waiting for the next tick,
 * and so does a view that starts reading a running clock: on a microtask,
 * outside the view's own rendering, with the next tick one minute after it.
 */
export const minuteClock: Readable<number> = {
  subscribe(run, invalidate) {
    const running = catchUp !== null;
    const unsubscribe = clock.subscribe(run, invalidate);
    if (running) queueMicrotask(() => catchUp?.());
    return unsubscribe;
  },
};
