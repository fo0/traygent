import { readFileSync, writeFileSync } from 'node:fs';

// Traygent artwork: a violet rounded tile with a dark robot head (antenna,
// ears, two eyes and a mouth cut out in the tile colour). The 64-unit
// geometry is the source: no existing raster/external asset is read.
/** @typedef {{ x: number, y: number, width: number, height: number, radius: number }} Box */
/** @type {Box} */
const tile = { x: 4, y: 4, width: 56, height: 56, radius: 14 };
/** Dark parts: head, ears and the antenna ball. */
/** @type {Box[]} */
const boxes = [
  { x: 15, y: 24, width: 34, height: 25, radius: 7 },
  { x: 10, y: 31, width: 6, height: 11, radius: 2 },
  { x: 48, y: 31, width: 6, height: 11, radius: 2 },
];
/** @type {[cx: number, cy: number, r: number][]} */
const knobs = [[32, 13, 4]];
/** @type {[x1: number, y1: number, x2: number, y2: number][]} */
const antenna = [[32, 15, 32, 24]];
const antennaWidth = 4;
/** Cut-outs in the head, drawn in the tile colour: eyes and the mouth. */
/** @type {[cx: number, cy: number, r: number][]} */
const eyes = [
  [25, 34, 4],
  [39, 34, 4],
];
/** @type {[x1: number, y1: number, x2: number, y2: number][]} */
const mouth = [[26, 42.5, 38, 42.5]];
const mouthWidth = 3;
/** @type {[null, number[], number[]]} Indexed by the `sample` result. */
const colors = [null, [185, 165, 255], [24, 18, 40]];
const samples = 4;
const outputDirectory = new URL('../src-tauri/icons/', import.meta.url);

/**
 * @param {number} x
 * @param {number} y
 * @param {Box} box
 */
function inBox(x, y, box) {
  if (x < box.x || y < box.y || x > box.x + box.width || y > box.y + box.height)
    return false;
  const cx = Math.max(
    box.x + box.radius,
    Math.min(x, box.x + box.width - box.radius),
  );
  const cy = Math.max(
    box.y + box.radius,
    Math.min(y, box.y + box.height - box.radius),
  );
  return (x - cx) ** 2 + (y - cy) ** 2 <= box.radius ** 2;
}

/**
 * @param {number} x
 * @param {number} y
 * @param {[number, number, number]} circle
 */
function inCircle(x, y, [cx, cy, r]) {
  return (x - cx) ** 2 + (y - cy) ** 2 <= r ** 2;
}

/**
 * @param {number} x
 * @param {number} y
 * @param {[number, number, number, number]} segment
 * @param {number} width
 */
function onSegment(x, y, [x1, y1, x2, y2], width) {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const t = Math.max(
    0,
    Math.min(1, ((x - x1) * dx + (y - y1) * dy) / (dx * dx + dy * dy)),
  );
  return (x - x1 - t * dx) ** 2 + (y - y1 - t * dy) ** 2 <= (width / 2) ** 2;
}

/**
 * @param {number} x
 * @param {number} y
 */
function sample(x, y) {
  if (!inBox(x, y, tile)) return 0;
  const cutOut =
    eyes.some((eye) => inCircle(x, y, eye)) ||
    mouth.some((line) => onSegment(x, y, line, mouthWidth));
  const dark =
    boxes.some((box) => inBox(x, y, box)) ||
    knobs.some((knob) => inCircle(x, y, knob)) ||
    antenna.some((line) => onSegment(x, y, line, antennaWidth));
  return dark && !cutOut ? 2 : 1;
}

const crcTable = Array.from({ length: 256 }, (_, value) => {
  for (let bit = 0; bit < 8; bit++)
    value = value & 1 ? 0xedb88320 ^ (value >>> 1) : value >>> 1;
  return value >>> 0;
});

/**
 * @param {string} type
 * @param {Buffer} data
 */
function chunk(type, data) {
  const content = Buffer.concat([Buffer.from(type), data]);
  let crc = 0xffffffff;
  for (const value of content)
    crc = (crcTable[(crc ^ value) & 255] ?? 0) ^ (crc >>> 8);
  const header = Buffer.alloc(4);
  header.writeUInt32BE(data.length);
  const checksum = Buffer.alloc(4);
  checksum.writeUInt32BE((crc ^ 0xffffffff) >>> 0);
  return Buffer.concat([header, content, checksum]);
}

// Stored DEFLATE blocks avoid compressor/version/platform-dependent output.
/** @param {Buffer} data */
function zlibStore(data) {
  /** @type {Buffer[]} */
  const parts = [Buffer.from([0x78, 0x01])];
  for (let offset = 0; offset < data.length; offset += 65535) {
    const block = data.subarray(offset, offset + 65535);
    const header = Buffer.alloc(5);
    header[0] = offset + block.length === data.length ? 1 : 0;
    header.writeUInt16LE(block.length, 1);
    header.writeUInt16LE(block.length ^ 0xffff, 3);
    parts.push(header, block);
  }
  let a = 1;
  let b = 0;
  for (const value of data) {
    a = (a + value) % 65521;
    b = (b + a) % 65521;
  }
  const adler = Buffer.alloc(4);
  adler.writeUInt32BE(((b << 16) | a) >>> 0);
  return Buffer.concat([...parts, adler]);
}

/** @param {number} size */
function png(size) {
  const stride = 1 + size * 4;
  const pixels = Buffer.alloc(stride * size);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      /** @type {[number, number, number]} */
      const counts = [0, 0, 0];
      for (let sy = 0; sy < samples; sy++)
        for (let sx = 0; sx < samples; sx++)
          counts[
            sample(
              ((x + (sx + 0.5) / samples) * 64) / size,
              ((y + (sy + 0.5) / samples) * 64) / size,
            )
          ]++;
      const covered = counts[1] + counts[2];
      const rgba = covered
        ? [
            ...colors[1].map((value, channel) =>
              Math.round(
                (value * counts[1] + (colors[2][channel] ?? 0) * counts[2]) /
                  covered,
              ),
            ),
            Math.round((covered * 255) / samples ** 2),
          ]
        : [0, 0, 0, 0];
      pixels.set(rgba, y * stride + 1 + x * 4);
    }
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(size);
  header.writeUInt32BE(size, 4);
  header[8] = 8;
  header[9] = 6; // RGBA; each scanline starts with filter 0.
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk('IHDR', header),
    chunk('IDAT', zlibStore(pixels)),
    chunk('IEND', Buffer.alloc(0)),
  ]);
}

/** @param {[size: number, image: Buffer][]} images */
function ico(images) {
  const header = Buffer.alloc(6 + 16 * images.length);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(images.length, 4);
  let offset = header.length;
  images.forEach(([size, image], index) => {
    const position = 6 + index * 16;
    header[position] = size === 256 ? 0 : size;
    header[position + 1] = size === 256 ? 0 : size;
    header.writeUInt16LE(1, position + 4);
    header.writeUInt16LE(32, position + 6);
    header.writeUInt32LE(image.length, position + 8);
    header.writeUInt32LE(offset, position + 12);
    offset += image.length;
  });
  return Buffer.concat([header, ...images.map(([, image]) => image)]);
}

/**
 * @param {Box} box
 * @param {number[]} [fill] Left out, the rect takes its group's fill.
 */
const rect = (box, fill) =>
  `<rect x="${box.x}" y="${box.y}" width="${box.width}" height="${box.height}" rx="${box.radius}"${fill ? ` fill="rgb(${fill})"` : ''}/>`;

function svg() {
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64">
  ${rect(tile, colors[1])}
  <g fill="rgb(${colors[2]})">
${boxes.map((box) => `    ${rect(box)}`).join('\n')}
${knobs.map(([cx, cy, r]) => `    <circle cx="${cx}" cy="${cy}" r="${r}"/>`).join('\n')}
  </g>
  <g stroke="rgb(${colors[2]})" stroke-width="${antennaWidth}">
${antenna.map(([x1, y1, x2, y2]) => `    <line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}"/>`).join('\n')}
  </g>
  <g fill="rgb(${colors[1]})">
${eyes.map(([cx, cy, r]) => `    <circle cx="${cx}" cy="${cy}" r="${r}"/>`).join('\n')}
  </g>
  <g stroke="rgb(${colors[1]})" stroke-width="${mouthWidth}" stroke-linecap="round">
${mouth.map(([x1, y1, x2, y2]) => `    <line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}"/>`).join('\n')}
  </g>
</svg>
`;
}

/** @type {[size: number, image: Buffer][]} */
const images = [16, 32, 48, 64, 128, 256].map((size) => [size, png(size)]);
/** @type {Map<string, Buffer>} */
const outputs = new Map([
  ...images
    .filter(([size]) => [32, 128, 256].includes(size))
    .map(
      /** @returns {[string, Buffer]} */
      ([size, data]) => [`${size}x${size}.png`, data],
    ),
  ['icon.ico', ico(images)],
  ['source.svg', Buffer.from(svg())],
]);
const check = process.argv[2] === '--check';
if (process.argv.length > (check ? 3 : 2))
  throw new Error('Usage: node scripts/generate-icons.mjs [--check]');
/** @type {string[]} */
const mismatches = [];
for (const [name, data] of outputs) {
  const destination = new URL(name, outputDirectory);
  if (!check) writeFileSync(destination, data);
  else {
    try {
      if (!readFileSync(destination).equals(data)) mismatches.push(name);
    } catch {
      mismatches.push(name);
    }
  }
}
if (mismatches.length)
  throw new Error(`Regenerate Traygent icons: ${mismatches.join(', ')}`);
console.log(
  `${check ? 'Verified' : 'Generated'} ${outputs.size} Traygent icon files`,
);
