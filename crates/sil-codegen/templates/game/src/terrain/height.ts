import type { GameManifest, HeightLayer } from "../manifest.ts";
import { scratchV3a } from "../core/pools.ts";

/** Hash-based 2D value noise — deterministic, no allocations. */
function hash2(x: number, y: number): number {
  let h = x * 374761393 + y * 668265263;
  h = (h ^ (h >> 13)) * 1274126177;
  return ((h ^ (h >> 16)) & 0xffff) / 65535;
}

function smoothNoise(x: number, y: number): number {
  const ix = Math.floor(x);
  const iy = Math.floor(y);
  const fx = x - ix;
  const fy = y - iy;
  const sx = fx * fx * (3 - 2 * fx);
  const sy = fy * fy * (3 - 2 * fy);
  const a = hash2(ix, iy);
  const b = hash2(ix + 1, iy);
  const c = hash2(ix, iy + 1);
  const d = hash2(ix + 1, iy + 1);
  const ab = a + (b - a) * sx;
  const cd = c + (d - c) * sx;
  return ab + (cd - ab) * sy;
}

function fbm(x: number, y: number, octaves: number, lacunarity: number, gain: number): number {
  let amp = 1;
  let freq = 1;
  let sum = 0;
  let norm = 0;
  for (let i = 0; i < octaves; i++) {
    sum += smoothNoise(x * freq, y * freq) * amp;
    norm += amp;
    amp *= gain;
    freq *= lacunarity;
  }
  return sum / norm;
}

/** Ridged FBM — sharp wind-carved crests. */
function ridged(x: number, y: number, octaves: number): number {
  let amp = 1;
  let freq = 1;
  let sum = 0;
  let norm = 0;
  for (let i = 0; i < octaves; i++) {
    const n = 1 - Math.abs(smoothNoise(x * freq, y * freq) * 2 - 1);
    sum += n * n * amp;
    norm += amp;
    amp *= 0.5;
    freq *= 2.15;
  }
  return sum / norm;
}

export type HeightFieldParams = {
  windRad: number;
  windCos: number;
  windSin: number;
  layers: HeightLayer[];
};

export function buildHeightParams(manifest: GameManifest): HeightFieldParams {
  const windDeg = manifest.terrain?.windDeg ?? 45;
  const windRad = (windDeg * Math.PI) / 180;
  const layers = manifest.terrain?.layers ?? [
    { kind: "dune", scaleM: 80, amplitudeM: 6 },
    { kind: "drift", scaleM: 12, amplitudeM: 1.2 },
    { kind: "sastrugi", scaleM: 2.5, amplitudeM: 0.15 },
  ];
  return {
    windRad,
    windCos: Math.cos(windRad),
    windSin: Math.sin(windRad),
    layers,
  };
}

function shearCoords(x: number, z: number, shear: number, cos: number, sin: number): [number, number] {
  const sx = x * cos - z * sin;
  const sz = x * sin + z * cos;
  return [sx + sz * shear, sz];
}

export function sampleHeight(x: number, z: number, params: HeightFieldParams): number {
  let h = 0;
  for (let i = 0; i < params.layers.length; i++) {
    const layer = params.layers[i]!;
    const scale = 1 / Math.max(layer.scaleM, 0.1);
    let shear = layer.shear ?? 0;
    let octaves = 3;
    let kindAmp = layer.amplitudeM;

    if (layer.kind === "dune") {
      shear = layer.shear ?? 0.08;
      octaves = 4;
      const [sx, sz] = shearCoords(x, z, shear, params.windCos, params.windSin);
      // Asymmetric dunes: steeper lee face via cubed fbm.
      const n = fbm(sx * scale, sz * scale, octaves, 2.05, 0.5);
      const shaped = Math.pow(n, 1.35);
      h += (shaped - 0.42) * 2.2 * kindAmp;
      continue;
    }

    if (layer.kind === "drift") {
      shear = layer.shear ?? 0.22;
      octaves = 4;
      const [sx, sz] = shearCoords(x, z, shear, params.windCos, params.windSin);
      const n = fbm(sx * scale, sz * scale, octaves, 2.2, 0.52);
      h += (n - 0.5) * 2.0 * kindAmp;
      continue;
    }

    if (layer.kind === "sastrugi") {
      shear = layer.shear ?? 0.4;
      octaves = 5;
      const [sx, sz] = shearCoords(x, z, shear, params.windCos, params.windSin);
      // Wind-aligned ridged grooves.
      const r = ridged(sx * scale, sz * scale * 1.6, octaves);
      h += (r - 0.45) * 2.4 * kindAmp;
      continue;
    }

    if (layer.kind === "ripple") {
      shear = layer.shear ?? 0.18;
      octaves = 3;
      const [sx, sz] = shearCoords(x, z, shear, params.windCos, params.windSin);
      const n = fbm(sx * scale * 1.4, sz * scale, octaves, 2.4, 0.45);
      h += (n - 0.5) * 2.0 * kindAmp;
      continue;
    }

    const [sx, sz] = shearCoords(x, z, shear, params.windCos, params.windSin);
    const n = fbm(sx * scale, sz * scale, octaves, 2.1, 0.48);
    h += (n - 0.5) * 2 * kindAmp;
  }
  return h;
}

export function sampleHeightNormal(
  x: number,
  z: number,
  params: HeightFieldParams,
  out: { x: number; y: number; z: number },
  epsilon = 0.15,
): void {
  const hL = sampleHeight(x - epsilon, z, params);
  const hR = sampleHeight(x + epsilon, z, params);
  const hD = sampleHeight(x, z - epsilon, params);
  const hU = sampleHeight(x, z + epsilon, params);
  out.x = hL - hR;
  out.y = 2 * epsilon;
  out.z = hD - hU;
  const len = Math.sqrt(out.x * out.x + out.y * out.y + out.z * out.z) || 1;
  out.x /= len;
  out.y /= len;
  out.z /= len;
}

/** Exposed rock outcrop bumps — sparse silhouette anchors. */
export function sampleRockOutcrop(x: number, z: number): number {
  const cx = Math.floor(x / 45) * 45 + 22;
  const cz = Math.floor(z / 45) * 45 + 22;
  const dx = x - cx;
  const dz = z - cz;
  const d2 = dx * dx + dz * dz;
  if (d2 > 64) return 0;
  const h = hash2(cx * 0.1, cz * 0.1);
  if (h < 0.82) return 0;
  const t = 1 - Math.sqrt(d2) / 8;
  return t * t * (2.5 + h * 3);
}

export function worldHeightAt(x: number, z: number, params: HeightFieldParams): number {
  return sampleHeight(x, z, params) + sampleRockOutcrop(x, z);
}

export function worldPositionAt(x: number, z: number, params: HeightFieldParams, target = scratchV3a): void {
  target.x = x;
  target.y = worldHeightAt(x, z, params);
  target.z = z;
}
