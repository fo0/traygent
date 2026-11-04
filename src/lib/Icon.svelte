<script module lang="ts">
  import { BRAND_GLYPHS, type BrandName } from './brandGlyphs';
  import { SIGNAL_BARS } from './optionMarks';

  // Traygent geometry constructed from basic SVG primitives; see docs/ASSETS.md.
  type Shape = {
    tag: 'line' | 'polyline' | 'rect' | 'circle' | 'path';
    attributes: Record<string, number | string>;
  };
  const line = (x1: number, y1: number, x2: number, y2: number): Shape => ({
    tag: 'line',
    attributes: { x1, y1, x2, y2 },
  });
  const wire = (points: string): Shape => ({
    tag: 'polyline',
    attributes: { points },
  });
  const ring = (cx: number, cy: number, r: number): Shape => ({
    tag: 'circle',
    attributes: { cx, cy, r },
  });
  const box = (
    x: number,
    y: number,
    width: number,
    height: number,
    rx = 2,
  ): Shape => ({ tag: 'rect', attributes: { x, y, width, height, rx } });
  const SHIELD = 'M12 3.5l7 2.6v5.4c0 4.2-2.9 7.6-7 9-4.1-1.4-7-4.8-7-9V6.1Z';

  const glyphs = {
    // The brand: a robot head with antenna, two eyes and a mouth.
    robot: [
      box(4.5, 8, 15, 12, 3),
      line(12, 5, 12, 8),
      ring(12, 3.8, 1.2),
      ring(9.3, 13.2, 1.3),
      ring(14.7, 13.2, 1.3),
      line(9.5, 17, 14.5, 17),
      line(2.5, 12.5, 2.5, 15.5),
      line(21.5, 12.5, 21.5, 15.5),
    ],
    folder: [
      {
        tag: 'path',
        attributes: {
          d: 'M3.5 7.5a2 2 0 0 1 2-2h4l2 2.5h7a2 2 0 0 1 2 2v7.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2Z',
        },
      },
    ],
    play: [wire('8,5 19,12 8,19 8,5')],
    settings: [
      ring(12, 12, 3),
      ring(12, 12, 7),
      line(12, 2, 12, 5),
      line(12, 19, 12, 22),
      line(2, 12, 5, 12),
      line(19, 12, 22, 12),
      line(5, 5, 7, 7),
      line(17, 17, 19, 19),
      line(5, 19, 7, 17),
      line(17, 7, 19, 5),
    ],
    down: [wire('5,8 12,15 19,8')],
    refresh: [
      wire('19,8 16,5 10,4 5,7 3,12 5,17 10,20 16,19 20,15'),
      wire('19,3 19,8 14,8'),
    ],
    check: [wire('4,12 9.5,17.5 20,7')],
    close: [line(5.5, 5.5, 18.5, 18.5), line(18.5, 5.5, 5.5, 18.5)],
    clock: [ring(12, 12, 8.5), wire('12,7.5 12,12 16,14')],
    // Three dots: the actions menu of a row.
    more: [ring(5.5, 12, 1.2), ring(12, 12, 1.2), ring(18.5, 12, 1.2)],
    external: [
      box(3.5, 8.5, 12, 12),
      wire('13,3.5 20.5,3.5 20.5,11'),
      line(20.5, 3.5, 10, 14),
    ],
    warning: [
      wire('12,3.5 21,20 3,20 12,3.5'),
      line(12, 8, 12, 13),
      line(12, 16.5, 12, 17),
    ],
    trash: [
      box(6, 7, 12, 14),
      line(4, 7, 20, 7),
      wire('9,7 9,3 15,3 15,7'),
      line(10, 11, 10, 17),
      line(14, 11, 14, 17),
    ],
    arrow: [line(4, 12, 20, 12), wire('14,6 20,12 14,18')],
    // A pencil: „Bearbeiten …“ of a „Zuletzt“ row.
    edit: [wire('4,20 4,16 15.5,4.5 19.5,8.5 8,20 4,20'), line(13, 7, 17, 11)],
    copy: [
      box(8.5, 8.5, 12, 12),
      {
        tag: 'path',
        attributes: {
          d: 'M15.5 8.5v-3a2 2 0 0 0-2-2h-8a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h3',
        },
      },
    ],
    info: [ring(12, 12, 8.5), line(12, 11, 12, 16), line(12, 8, 12, 8.01)],
    // Environments: Lokal and WSL (Docker uses its product mark).
    monitor: [
      box(3.5, 4.5, 17, 11.5),
      line(12, 16, 12, 19.5),
      line(8, 19.5, 16, 19.5),
    ],
    terminal: [
      box(3.5, 5, 17, 14),
      wire('7.5,10 10,12 7.5,14'),
      line(12, 14.5, 16, 14.5),
    ],
    // Permissions: Nachfragen, Auto, Bypass (Claude Code) / YOLO.
    shield: [{ tag: 'path', attributes: { d: SHIELD } }],
    shieldCheck: [
      { tag: 'path', attributes: { d: SHIELD } },
      wire('8.8,12.2 11,14.4 15.2,10.2'),
    ],
    bolt: [wire('13,3 6,13.5 11.5,13.5 10.5,21 18,10 12.5,10 13,3')],
    // Composer chips: Modell and Session (Denken uses `signal`).
    cpu: [
      box(6.5, 6.5, 11, 11),
      box(9.5, 9.5, 5, 5, 1),
      line(10, 3.5, 10, 6.5),
      line(14, 3.5, 14, 6.5),
      line(10, 17.5, 10, 20.5),
      line(14, 17.5, 14, 20.5),
      line(3.5, 10, 6.5, 10),
      line(3.5, 14, 6.5, 14),
      line(17.5, 10, 20.5, 10),
      line(17.5, 14, 20.5, 14),
    ],
    history: [
      { tag: 'path', attributes: { d: 'M4.5 12a7.5 7.5 0 1 0 2.2-5.3' } },
      wire('4.5,3.8 4.5,7.5 8.2,7.5'),
      wire('12,8 12,12 14.8,13.8'),
    ],
    // Ports: published host ⇄ container, and adding a custom one.
    ports: [
      line(4, 9, 19, 9),
      wire('15.5,5.5 19,9 15.5,12.5'),
      line(20, 15, 5, 15),
      wire('8.5,11.5 5,15 8.5,18.5'),
    ],
    plus: [line(12, 5, 12, 19), line(5, 12, 19, 12)],
    // A repository: a branch off its main line.
    repo: [
      ring(7, 5.5, 2),
      ring(7, 18.5, 2),
      ring(17, 7.5, 2),
      line(7, 7.5, 7, 16.5),
      { tag: 'path', attributes: { d: 'M17 9.5c0 4-10 3-10 7' } },
    ],
    // Darstellung: Hell and Dunkel (System uses `monitor`).
    sun: [
      ring(12, 12, 4),
      line(12, 2.5, 12, 4.5),
      line(12, 19.5, 12, 21.5),
      line(2.5, 12, 4.5, 12),
      line(19.5, 12, 21.5, 12),
      line(5.3, 5.3, 6.7, 6.7),
      line(17.3, 17.3, 18.7, 18.7),
      line(5.3, 18.7, 6.7, 17.3),
      line(17.3, 6.7, 18.7, 5.3),
    ],
    moon: [
      {
        tag: 'path',
        attributes: { d: 'M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5Z' },
      },
    ],
    // A private repository.
    lock: [
      box(5.5, 10.5, 13, 9.5),
      { tag: 'path', attributes: { d: 'M8.5 10.5V8a3.5 3.5 0 0 1 7 0v2.5' } },
    ],
  } satisfies Record<string, Shape[]>;

  /**
   * Every glyph name, line glyphs and product marks alike; a typo at a call
   * site is a type error, not a silent fallback.
   */
  export type IconName = keyof typeof glyphs | BrandName | 'signal';

  /** The bars of `signal`, left to right. */
  const BARS = Array.from({ length: SIGNAL_BARS }, (_, bar) => bar);

  const isBrand = (name: IconName): name is BrandName =>
    Object.hasOwn(BRAND_GLYPHS, name);
</script>

<script lang="ts">
  let {
    name,
    size = 18,
    class: className = '',
    level = 0,
  }: {
    name: IconName;
    size?: number;
    class?: string;
    /** `signal` only: how many bars are filled (`optionMarks → effortLevel`). */
    level?: number;
  } = $props();
</script>

{#if isBrand(name)}
  <!-- A product mark: filled; its colour comes from `.mark-<name>`. -->
  <svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="currentColor"
    fill-rule="evenodd"
    class={['mark', `mark-${name}`, className]}
    aria-hidden="true"><path d={BRAND_GLYPHS[name]} /></svg
  >
{:else if name === 'signal'}
  <!-- The thinking level: `level` bars filled, the rest dim. Its own 14-unit
       grid gives each bar 2 px and each gap 1 px at 14 px. -->
  <svg
    width={size}
    height={size}
    viewBox="0 0 14 14"
    fill="currentColor"
    class={['signal', className]}
    aria-hidden="true"
  >
    {#each BARS as bar (bar)}<rect
        x={bar * 3}
        y={10 - bar * 2}
        width="2"
        height={3 + bar * 2}
        rx="0.5"
        opacity={bar < level ? 1 : 0.3}
      />{/each}
  </svg>
{:else}<svg
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.6"
    stroke-linecap="round"
    stroke-linejoin="round"
    class={className}
    aria-hidden="true"
  >
    {#each glyphs[name] ?? glyphs.robot as shape, index (index)}
      <svelte:element this={shape.tag} {...shape.attributes} />
    {/each}
  </svg>
{/if}
