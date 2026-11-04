// 生成应用图标源图（512x512）。无外部依赖，手写 PNG 编码。
// 运行：node scripts/gen-icon.mjs
import fs from "node:fs";
import zlib from "node:zlib";

const S = 512;
const R = 100;
const BG = [13, 110, 102, 255];
const FG = [240, 253, 250, 255];

const insideRound = (x, y) => {
  const cx = Math.min(Math.max(x, R), S - R);
  const cy = Math.min(Math.max(y, R), S - R);
  const dx = x - cx;
  const dy = y - cy;
  return dx * dx + dy * dy <= R * R;
};

const inBox = (x, y, x0, y0, x1, y1) => x >= x0 && x < x1 && y >= y0 && y < y1;

const table = (() => {
  const t = new Int32Array(256);
  for (let i = 0; i < 256; i++) {
    let c = i;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[i] = c;
  }
  return t;
})();

const crc32 = (buf) => {
  let c = -1;
  for (const b of buf) c = table[(c ^ b) & 0xff] ^ (c >>> 8);
  return (c ^ -1) >>> 0;
};

const chunk = (type, data) => {
  const len = Buffer.alloc(4);
  len.writeUInt32BE(data.length);
  const td = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(td));
  return Buffer.concat([len, td, crc]);
};

const rows = [];
for (let y = 0; y < S; y++) {
  const row = Buffer.alloc(1 + S * 4);
  row[0] = 0;
  for (let x = 0; x < S; x++) {
    let c = [0, 0, 0, 0];
    if (insideRound(x + 0.5, y + 0.5)) c = BG;
    if (
      inBox(x, y, 124, 124, 228, 228) ||
      inBox(x, y, 284, 124, 388, 228) ||
      inBox(x, y, 124, 284, 388, 388)
    ) {
      c = FG;
    }
    row[1 + x * 4] = c[0];
    row[2 + x * 4] = c[1];
    row[3 + x * 4] = c[2];
    row[4 + x * 4] = c[3];
  }
  rows.push(row);
}

const ihdr = Buffer.alloc(13);
ihdr.writeUInt32BE(S, 0);
ihdr.writeUInt32BE(S, 4);
ihdr[8] = 8;
ihdr[9] = 6;
ihdr[10] = 0;
ihdr[11] = 0;
ihdr[12] = 0;

const png = Buffer.concat([
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
  chunk("IHDR", ihdr),
  chunk("IDAT", zlib.deflateSync(Buffer.concat(rows), { level: 9 })),
  chunk("IEND", Buffer.alloc(0)),
]);

fs.writeFileSync("icon-src.png", png);
console.log(`icon-src.png 已生成，${png.length} 字节`);
