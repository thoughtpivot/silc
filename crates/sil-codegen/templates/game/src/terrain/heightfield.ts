/**
 * Baked macro heightfield: one RG float texture + CPU mirror for grounding.
 * Bake runs once at load from the same procedural noise as height.ts so
 * character grounding never disagrees with the drawn surface.
 */
import {
  Constants,
  RawTexture,
  Scene,
  Texture,
  Vector2,
} from "@babylonjs/core";
import type { GameManifest } from "../manifest.ts";
import {
  buildHeightParams,
  worldHeightAt,
  sampleRockOutcrop,
  type HeightFieldParams,
} from "./height.ts";

export const WORLD_SIZE = 2048;
/** 512² — dunes already read under mid-distance camera; 1024 kept for later knobs. */
export const HEIGHT_RES = 512;
export const PLAY_RADIUS = 620;

export class Heightfield {
  readonly origin = new Vector2(-WORLD_SIZE / 2, -WORLD_SIZE / 2);
  readonly size = WORLD_SIZE;
  readonly texelWorld: number;
  readonly heightParams: HeightFieldParams;
  readonly heightTex: RawTexture;
  heightCPU: Float32Array | null = null;
  cpuRes = 0;
  cpuTexel = 1;
  minHeight = 0;
  maxHeight = 8;
  private readonly heightData: Float32Array;

  constructor(scene: Scene, manifest: GameManifest) {
    this.heightParams = buildHeightParams(manifest);
    this.texelWorld = WORLD_SIZE / HEIGHT_RES;
    this.heightData = new Float32Array(HEIGHT_RES * HEIGHT_RES * 4);
    this.heightTex = RawTexture.CreateRGBATexture(
      this.heightData,
      HEIGHT_RES,
      HEIGHT_RES,
      scene,
      false,
      false,
      Texture.BILINEAR_SAMPLINGMODE,
      Constants.TEXTURETYPE_FLOAT,
    );
    this.heightTex.wrapU = Texture.CLAMP_ADDRESSMODE;
    this.heightTex.wrapV = Texture.CLAMP_ADDRESSMODE;
  }

  /** Fill texture + CPU mirror. Chunked with setTimeout yields (not rAF). */
  async bake(onProgress?: (label: string, pct: number) => void): Promise<void> {
    const amp = 1;
    let minH = Infinity;
    let maxH = -Infinity;
    const chunk = 32;
    for (let y0 = 0; y0 < HEIGHT_RES; y0 += chunk) {
      const y1 = Math.min(HEIGHT_RES, y0 + chunk);
      for (let y = y0; y < y1; y++) {
        const wz = this.origin.y + ((y + 0.5) / HEIGHT_RES) * this.size;
        for (let x = 0; x < HEIGHT_RES; x++) {
          const wx = this.origin.x + ((x + 0.5) / HEIGHT_RES) * this.size;
          const h = worldHeightAt(wx, wz, this.heightParams) * amp;
          const rock = sampleRockOutcrop(wx, wz) > 0.01 ? 1 : 0;
          const i = (y * HEIGHT_RES + x) * 4;
          this.heightData[i] = h;
          this.heightData[i + 1] = rock;
          this.heightData[i + 2] = 0;
          this.heightData[i + 3] = 1;
          if (h < minH) minH = h;
          if (h > maxH) maxH = h;
        }
      }
      if (onProgress) {
        onProgress("Baking heightfield", 0.15 + (y1 / HEIGHT_RES) * 0.35);
      }
      // Yield without depending on rAF (background tabs throttle rAF to ~0).
      await new Promise((r) => setTimeout(r, 0));
    }
    this.heightTex.update(this.heightData);
    this.minHeight = Number.isFinite(minH) ? minH : 0;
    this.maxHeight = Number.isFinite(maxH) ? maxH : 8;
    this._buildCpuMirror();
  }

  private _buildCpuMirror(): void {
    const res = HEIGHT_RES / 2;
    const dst = new Float32Array(res * res);
    for (let y = 0; y < res; y++) {
      const r0 = y * 2 * HEIGHT_RES;
      const r1 = (y * 2 + 1) * HEIGHT_RES;
      for (let x = 0; x < res; x++) {
        const c0 = x * 2;
        const c1 = c0 + 1;
                dst[y * res + x] =
                  (this.heightData[(r0 + c0) * 4]! +
                    this.heightData[(r0 + c1) * 4]! +
                    this.heightData[(r1 + c0) * 4]! +
                    this.heightData[(r1 + c1) * 4]!) *
                  0.25;
      }
    }
    this.heightCPU = dst;
    this.cpuRes = res;
    this.cpuTexel = this.size / res;
  }

  /** Bilinear sample of the CPU mirror — matches GPU bilinear of the bake. */
  heightAt(wx: number, wz: number): number {
    if (!this.heightCPU) {
      return worldHeightAt(wx, wz, this.heightParams);
    }
    const u = (wx - this.origin.x) / this.size;
    const v = (wz - this.origin.y) / this.size;
    const x = u * this.cpuRes - 0.5;
    const y = v * this.cpuRes - 0.5;
    const x0 = Math.floor(x);
    const y0 = Math.floor(y);
    const fx = x - x0;
    const fy = y - y0;
    const sample = (ix: number, iy: number): number => {
      const cx = Math.max(0, Math.min(this.cpuRes - 1, ix));
      const cy = Math.max(0, Math.min(this.cpuRes - 1, iy));
      return this.heightCPU![cy * this.cpuRes + cx]!;
    };
    const a = sample(x0, y0);
    const b = sample(x0 + 1, y0);
    const c = sample(x0, y0 + 1);
    const d = sample(x0 + 1, y0 + 1);
    return a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy;
  }

  dispose(): void {
    this.heightTex.dispose();
  }
}
