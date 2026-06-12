/**
 * Renders public/og-preview.png — reference neon coin + unified reference background + Hero typography.
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import sharp from 'sharp';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');
const refPath = path.join(root, 'public', 'og-reference-source.jpg');
const outPath = path.join(root, 'public', 'og-preview.png');
const fontBold = fs.readFileSync(path.join(__dirname, 'fonts', 'Inter-Bold.ttf'));
const fontRegular = fs.readFileSync(path.join(__dirname, 'fonts', 'Inter-Regular.ttf'));

const W = 1200;
const H = 630;

const PRIMARY = '#B847E0';
const FOREGROUND = '#F2F2F2';
const MUTED = '#999999';
const SHADOW = '#0A0810';

const HEADLINE_PX = 72;
const BODY_PX = 20;
const BODY_LH = 32;
const TRACKING = -0.025;

const COIN_SRC_W = 490;
const COIN_DISPLAY_H = 560;
const COIN_LEFT = 36;
const TEXT_LEFT = 532;
const ROW_TOP = 96;

const fontBoldB64 = fontBold.toString('base64');
const fontRegularB64 = fontRegular.toString('base64');

// Coin backdrop colour sampled from reference (≈ rgb 1,0,8) — solid across full OG, soft floor glow under coin only
const BG = '#010008';
const bgSvg = Buffer.from(`<?xml version="1.0" encoding="UTF-8"?>
<svg width="${W}" height="${H}" xmlns="http://www.w3.org/2000/svg">
  <rect width="${W}" height="${H}" fill="${BG}"/>
  <radialGradient id="floor" cx="20%" cy="88%" r="42%">
    <stop offset="0%" stop-color="#3B1A6E" stop-opacity="0.28"/>
    <stop offset="55%" stop-color="#1A0A30" stop-opacity="0.12"/>
    <stop offset="100%" stop-color="${BG}" stop-opacity="0"/>
  </radialGradient>
  <rect width="${W}" height="${H}" fill="url(#floor)"/>
</svg>`);
const bgBuf = await sharp(bgSvg).png().toBuffer();

const coinBuf = await sharp(refPath)
  .extract({ left: 0, top: 0, width: COIN_SRC_W, height: 682 })
  .resize(Math.round(COIN_SRC_W * (COIN_DISPLAY_H / 682)), COIN_DISPLAY_H, {
    kernel: sharp.kernel.lanczos3,
  })
  .sharpen({ sigma: 1.1, m1: 0.45, m2: 0.3 })
  .png()
  .toBuffer();

const coinMeta = await sharp(coinBuf).metadata();
const coinTop = Math.round((H - COIN_DISPLAY_H) / 2);

const textSvg = Buffer.from(`<?xml version="1.0" encoding="UTF-8"?>
<svg width="${W * 2}" height="${H * 2}" viewBox="0 0 ${W} ${H}" xmlns="http://www.w3.org/2000/svg">
  <defs>
    <style>
      @font-face {
        font-family: 'Inter';
        font-weight: 700;
        src: url('data:font/ttf;base64,${fontBoldB64}') format('truetype');
      }
      @font-face {
        font-family: 'Inter';
        font-weight: 400;
        src: url('data:font/ttf;base64,${fontRegularB64}') format('truetype');
      }
    </style>
    <filter id="bodyShadow" x="-8%" y="-8%" width="116%" height="120%">
      <feDropShadow dx="0" dy="1" stdDeviation="4" flood-color="${SHADOW}" flood-opacity="0.65"/>
    </filter>
    <filter id="headShadow" x="-4%" y="-4%" width="108%" height="112%">
      <feDropShadow dx="0" dy="1" stdDeviation="6" flood-color="${SHADOW}" flood-opacity="0.7"/>
    </filter>
  </defs>

  <text x="${TEXT_LEFT}" y="${ROW_TOP + 64}" fill="none" stroke="${PRIMARY}" stroke-width="2"
    font-family="Inter, sans-serif" font-size="${HEADLINE_PX}" font-weight="700"
    letter-spacing="${HEADLINE_PX * TRACKING}" filter="url(#headShadow)">Turn Math Into</text>
  <text x="${TEXT_LEFT}" y="${ROW_TOP + 64}" fill="${FOREGROUND}"
    font-family="Inter, sans-serif" font-size="${HEADLINE_PX}" font-weight="700"
    letter-spacing="${HEADLINE_PX * TRACKING}" filter="url(#headShadow)">Turn Math Into</text>

  <text x="${TEXT_LEFT}" y="${ROW_TOP + 144}" fill="none" stroke="${PRIMARY}" stroke-width="2"
    font-family="Inter, sans-serif" font-size="${HEADLINE_PX}" font-weight="700"
    letter-spacing="${HEADLINE_PX * TRACKING}" filter="url(#headShadow)">$BEANS</text>
  <text x="${TEXT_LEFT}" y="${ROW_TOP + 144}" fill="${PRIMARY}"
    font-family="Inter, sans-serif" font-size="${HEADLINE_PX}" font-weight="700"
    letter-spacing="${HEADLINE_PX * TRACKING}" filter="url(#headShadow)">$BEANS</text>

  <text fill="${FOREGROUND}" font-family="Inter, sans-serif" font-size="${BODY_PX}" font-weight="400"
    filter="url(#bodyShadow)">
    <tspan x="${TEXT_LEFT}" y="${ROW_TOP + 196}">COINjecture pays for hard math on-chain —</tspan>
    <tspan x="${TEXT_LEFT}" dy="${BODY_LH}">mine blocks for emission, solve marketplace</tspan>
    <tspan x="${TEXT_LEFT}" dy="${BODY_LH}">bounties for escrowed payouts, and turn</tspan>
    <tspan x="${TEXT_LEFT}" dy="${BODY_LH}">verified NP work into real token value.</tspan>
  </text>

  <text x="${TEXT_LEFT}" y="${H - 40}" fill="${MUTED}"
    font-family="Inter, sans-serif" font-size="14" font-weight="400" filter="url(#bodyShadow)">coinjecture.com · Pre-audit testnet</text>
</svg>`);

const textLayer = await sharp(textSvg)
  .resize(W, H, { kernel: sharp.kernel.lanczos3 })
  .png()
  .toBuffer();

await sharp(bgBuf)
  .composite([
    { input: coinBuf, left: COIN_LEFT, top: coinTop },
    { input: textLayer, left: 0, top: 0 },
  ])
  .png({ compressionLevel: 6, effort: 10 })
  .toFile(outPath);

console.log(`Wrote ${outPath} (unified ref background ${W}×${H}, coin ${coinMeta.width}×${COIN_DISPLAY_H})`);
