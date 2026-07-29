/**
 * GPU terrain deformation — RGBA16F ping-pong with a strict read/write split.
 *
 * Beauty ALWAYS samples `texture` (the completed read target).
 * The sim ALWAYS writes the opposite target, then we swap after render().
 * Never bind the attachment being written as a sampler in the same pass —
 * that WebGPU hazard blanked every frame previously.
 */
import {
  Constants,
  ProceduralTexture,
  RawTexture,
  Scene,
  ShaderLanguage,
  ShaderStore,
  Texture,
  Vector2,
} from "@babylonjs/core";
import type { GameManifest } from "../manifest.ts";
import { whenReady } from "../core/gpuUtil.ts";

const BRUSH_ROWS = 3;
const MAX_BRUSHES = 96;
const RELAX_STEP = 0.4;

const DEFORM_SIM = `
varying vUV : vec2<f32>;

uniform prevCenter : vec2<f32>;
uniform curCenter : vec2<f32>;
uniform size : f32;
uniform invRes : f32;
uniform relaxDt : f32;
uniform brushCount : f32;
uniform refillRate : f32;
uniform windDir : vec2<f32>;

var prevState : texture_2d<f32>;
var prevStateSampler : sampler;
var brushTex : texture_2d<f32>;
var brushTexSampler : sampler;

fn texelWorld(uv : vec2<f32>, centre : vec2<f32>, size : f32) -> vec2<f32> {
  var base : vec2<f32> = uv * size;
  return base + size * round((centre - base) / size);
}

@fragment
fn main(input : FragmentInputs) -> FragmentOutputs {
  var uv : vec2<f32> = input.vUV;
  var world : vec2<f32> = texelWorld(uv, uniforms.curCenter, uniforms.size);
  var halfSize : f32 = uniforms.size * 0.5;
  var wasInside : bool = all(abs(world - uniforms.prevCenter) <= vec2<f32>(halfSize));

  var state : vec4<f32> = vec4<f32>(0.0);
  if (wasInside) {
    state = textureSampleLevel(prevState, prevStateSampler, uv, 0.0);
    if (uniforms.relaxDt > 0.0) {
      var e : f32 = uniforms.invRes;
      var n : vec4<f32> = textureSampleLevel(prevState, prevStateSampler, uv + vec2<f32>(0.0, e), 0.0);
      var s : vec4<f32> = textureSampleLevel(prevState, prevStateSampler, uv + vec2<f32>(0.0, -e), 0.0);
      var w : vec4<f32> = textureSampleLevel(prevState, prevStateSampler, uv + vec2<f32>(-e, 0.0), 0.0);
      var ea : vec4<f32> = textureSampleLevel(prevState, prevStateSampler, uv + vec2<f32>(e, 0.0), 0.0);
      var k : f32 = clamp(uniforms.refillRate * uniforms.relaxDt, 0.0, 1.0);
      var kDep : f32 = min(0.22, 0.004 * k);
      var kBerm : f32 = min(0.22, 0.012 * k);
      var lapDep : f32 = (n.x + s.x + w.x + ea.x) - 4.0 * state.x;
      var lapBerm : f32 = (n.y + s.y + w.y + ea.y) - 4.0 * state.y;
      state.x = state.x + lapDep * kDep;
      state.y = state.y + lapBerm * kBerm;
      var decay : f32 = exp(-uniforms.refillRate * uniforms.relaxDt * 0.8);
      state.x = state.x * decay;
      state.y = state.y * decay;
      state.z = state.z * mix(1.0, decay, 0.35);
      state.w = state.w * mix(1.0, decay, 0.2);
    }
  }

  var count : i32 = i32(uniforms.brushCount);
  for (var i : i32 = 0; i < 96; i++) {
    if (i >= count) { break; }
    var r0 : vec4<f32> = textureLoad(brushTex, vec2<i32>(i, 0), 0);
    var r1 : vec4<f32> = textureLoad(brushTex, vec2<i32>(i, 1), 0);
    var r2 : vec4<f32> = textureLoad(brushTex, vec2<i32>(i, 2), 0);
    var radius : f32 = max(r0.z, 0.01);
    var elong : f32 = max(r0.w, 1.0);
    var d : vec2<f32> = world - vec2<f32>(r0.x, r0.y);
    var lx : f32 = (d.x * r1.x + d.y * r1.y) / elong;
    var lz : f32 = -d.x * r1.y + d.y * r1.x;
    var dist : f32 = length(vec2<f32>(lx, lz));
    var t : f32 = dist / radius;
    if (t < 1.35) {
      var core : f32 = 1.0 - smoothstep(0.0, 1.0, t);
      var floorW : f32 = smoothstep(0.15, 0.85, core);
      state.x = min(0.65, state.x + r1.z * floorW);
      var ring : f32 = exp(-pow((t - 1.05) / 0.28, 2.0)) * step(0.55, t);
      state.y = min(0.4, state.y + r1.w * ring * r1.z);
      state.z = max(state.z, r2.x * core);
      state.w = max(state.w, r2.y * core + r2.x * core * 0.15);
    }
  }

  state = clamp(state, vec4<f32>(0.0), vec4<f32>(0.65, 0.4, 1.0, 1.0));
  fragmentOutputs.color = state;
}
`;

export type DeformationChannels = {
  depression: number;
  displacedMass: number;
  compression: number;
  wetness: number;
  ice: number;
};

export class DeformationField {
  readonly extentM: number;
  readonly resolution: number;
  readonly texelM: number;
  /** Completed read target — safe for beauty shaders to sample. */
  texture: Texture;
  /** True once GPU ping-pong targets are live. */
  get usingGpu(): boolean {
    return this.gpuOk;
  }
  centerX = 0;
  centerZ = 0;
  private gpuOk = false;
  private readonly scene: Scene;
  private readonly refillRate: number;
  private readonly brushData: Float32Array;
  private brushCount = 0;
  private readonly brushTex: RawTexture;
  private readonly targets: ProceduralTexture[] = [];
  /** Index of the completed read buffer (beauty samples this). */
  private read = 0;
  private readonly prevCenter = new Vector2(1e6, 1e6);
  private readonly curCenter = new Vector2(0, 0);
  private relaxOwed = 0;
  private windDir = new Vector2(0.7, 0.3);
  private readonly cpuData: Float32Array;
  private readonly cpuTex: RawTexture;
  private warmed = false;
  private readonly cpuApprox = new Map<string, number>();

  constructor(scene: Scene, manifest: GameManifest) {
    this.scene = scene;
    const extentM = manifest.deformation?.extentM ?? 80;
    const authoredRes = manifest.deformation?.resolution;
    const texelCm = manifest.deformation?.texelCm ?? 4;
    this.extentM = extentM;
    this.texelM = texelCm / 100;
    this.resolution = Math.min(
      512,
      Math.max(
        256,
        authoredRes !== undefined
          ? Math.round(authoredRes)
          : Math.round(extentM / this.texelM),
      ),
    );
    this.refillRate = manifest.deformation?.refillRate ?? 0.00015;

    ShaderStore.ShadersStoreWGSL["deformSimPixelShader"] = DEFORM_SIM;

    this.brushData = new Float32Array(MAX_BRUSHES * BRUSH_ROWS * 4);
    this.brushTex = RawTexture.CreateRGBATexture(
      this.brushData,
      MAX_BRUSHES,
      BRUSH_ROWS,
      scene,
      false,
      false,
      Constants.TEXTURE_NEAREST_SAMPLINGMODE,
      Constants.TEXTURETYPE_FLOAT,
    );
    this.brushTex.wrapU = Texture.CLAMP_ADDRESSMODE;
    this.brushTex.wrapV = Texture.CLAMP_ADDRESSMODE;

    this.cpuData = new Float32Array(this.resolution * this.resolution * 4);
    this.cpuTex = RawTexture.CreateRGBATexture(
      this.cpuData,
      this.resolution,
      this.resolution,
      scene,
      false,
      false,
      Texture.BILINEAR_SAMPLINGMODE,
      Constants.TEXTURETYPE_FLOAT,
    );
    this.cpuTex.wrapU = Texture.WRAP_ADDRESSMODE;
    this.cpuTex.wrapV = Texture.WRAP_ADDRESSMODE;

    // Start on CPU so first beauty frames never touch an unfinished RT.
    this.texture = this.cpuTex;
    this.warmed = true;
  }

  private makeTarget(i: number): ProceduralTexture {
    const pt = new ProceduralTexture(
      `deform${i}`,
      { width: this.resolution, height: this.resolution },
      "deformSim",
      this.scene,
      {
        generateMipMaps: false,
        type: Constants.TEXTURETYPE_HALF_FLOAT,
        format: Constants.TEXTUREFORMAT_RGBA,
        samplingMode: Constants.TEXTURE_BILINEAR_SAMPLINGMODE,
        shaderLanguage: ShaderLanguage.WGSL,
        skipSceneRegistration: true,
      },
    );
    pt.wrapU = Constants.TEXTURE_WRAP_ADDRESSMODE;
    pt.wrapV = Constants.TEXTURE_WRAP_ADDRESSMODE;
    pt.refreshRate = 0;
    return pt;
  }

  /**
   * Promote to GPU ping-pong after the scene is live. Safe to call once.
   * Beauty keeps sampling CPU until both targets are ready and cleared.
   */
  async warmGpu(): Promise<void> {
    if (this.gpuOk) return;
    try {
      this.targets.push(this.makeTarget(0), this.makeTarget(1));
      for (const t of this.targets) {
        await whenReady(t, t.name, [], 10000);
        this.bindPass(t, this.cpuTex, 0);
        t.render();
      }
      this.read = 0;
      this.texture = this.targets[this.read]!;
      this.gpuOk = true;
      this.warmed = true;
    } catch (err) {
      console.warn("[deformation] GPU sim unavailable, staying on CPU:", err);
      for (const t of this.targets) t.dispose();
      this.targets.length = 0;
      this.gpuOk = false;
      this.texture = this.cpuTex;
    }
  }

  async warm(): Promise<void> {
    await this.warmGpu();
  }

  useCpuFallback(): void {
    this.gpuOk = false;
    this.texture = this.cpuTex;
    this.warmed = true;
  }

  follow(playerX: number, playerZ: number): void {
    const snap = this.texelM;
    const nextX = Math.round(playerX / snap) * snap;
    const nextZ = Math.round(playerZ / snap) * snap;
    this.prevCenter.copyFrom(this.curCenter);
    this.curCenter.set(nextX, nextZ);
    this.centerX = nextX;
    this.centerZ = nextZ;
  }

  brush(
    worldX: number,
    worldZ: number,
    radiusM: number,
    depthM: number,
    options?: {
      compression?: number;
      wetness?: number;
      ice?: number;
      shape?: string;
      yaw?: number;
      berm?: number;
      elongation?: number;
    },
  ): void {
    if (this.brushCount >= MAX_BRUSHES) return;
    const i = this.brushCount++;
    const yaw = options?.yaw ?? 0;
    const base = i * 4;
    this.brushData[base] = worldX;
    this.brushData[base + 1] = worldZ;
    this.brushData[base + 2] = radiusM;
    this.brushData[base + 3] = options?.elongation ?? (options?.shape === "groove" ? 2.2 : 1);
    const r1 = MAX_BRUSHES * 4 + base;
    this.brushData[r1] = Math.cos(yaw);
    this.brushData[r1 + 1] = Math.sin(yaw);
    this.brushData[r1 + 2] = depthM;
    this.brushData[r1 + 3] = options?.berm ?? depthM * 0.55;
    const r2 = MAX_BRUSHES * 8 + base;
    this.brushData[r2] = options?.compression ?? 0;
    this.brushData[r2 + 1] = (options?.ice ?? 0) + (options?.wetness ?? 0) * 0.5;
    this.brushData[r2 + 2] = 0.35;
    this.brushData[r2 + 3] = (worldX * 12.9898 + worldZ * 78.233) % 1;

    const key = `${Math.round(worldX)}_${Math.round(worldZ)}`;
    this.cpuApprox.set(key, (this.cpuApprox.get(key) ?? 0) + depthM * 0.5);
  }

  writeState(
    worldX: number,
    worldZ: number,
    radiusM: number,
    channelOrChannels: string | Partial<DeformationChannels>,
    amount = 0,
  ): void {
    const channels: Partial<DeformationChannels> =
      typeof channelOrChannels === "string"
        ? {
            depression: channelOrChannels === "depression" ? amount : 0.02,
            compression: channelOrChannels === "compression" ? amount : 0,
            wetness: channelOrChannels === "wetness" ? amount : 0,
            ice: channelOrChannels === "ice" ? amount : 0,
            displacedMass: channelOrChannels === "displacedMass" ? amount : 0.01,
          }
        : channelOrChannels;
    this.brush(worldX, worldZ, radiusM, channels.depression ?? 0.02, {
      compression: channels.compression,
      wetness: channels.wetness,
      ice: channels.ice,
      berm: channels.displacedMass ?? 0.01,
    });
  }

  refill(dt: number): void {
    this.relaxOwed += dt;
  }

  /**
   * Run one sim step. Beauty must sample `this.texture` only AFTER this returns
   * (engine update order: upload → then scene.render).
   */
  upload(): void {
    if (!this.warmed) return;
    if (!this.gpuOk || this.targets.length < 2) {
      this.applyBrushesCpu();
      if (this.relaxOwed > 0.05) {
        this.relaxCpu(this.relaxOwed);
        this.relaxOwed = 0;
      }
      this.cpuTex.update(this.cpuData);
      this.brushCount = 0;
      this.texture = this.cpuTex;
      return;
    }

    this.brushTex.update(this.brushData);
    let relax = 0;
    if (this.relaxOwed >= RELAX_STEP) {
      relax = RELAX_STEP;
      this.relaxOwed -= RELAX_STEP;
    }

    const readTex = this.targets[this.read]!;
    const writeIdx = 1 - this.read;
    const writeTex = this.targets[writeIdx]!;
    // Beauty still holds readTex; we only write writeTex.
    this.bindPass(writeTex, readTex, relax);
    writeTex.render();
    // Swap: completed write becomes the new read for the next beauty frame.
    this.read = writeIdx;
    this.texture = writeTex;
    this.brushCount = 0;
  }

  sampleDepression(worldX: number, worldZ: number): number {
    const key = `${Math.round(worldX)}_${Math.round(worldZ)}`;
    return this.cpuApprox.get(key) ?? 0;
  }

  private bindPass(target: ProceduralTexture, prev: Texture, relaxDt: number): void {
    target.setTexture("prevState", prev);
    target.setTexture("brushTex", this.brushTex);
    target.setVector2("prevCenter", this.prevCenter);
    target.setVector2("curCenter", this.curCenter);
    target.setFloat("size", this.extentM);
    target.setFloat("invRes", 1 / this.resolution);
    target.setFloat("relaxDt", relaxDt);
    target.setFloat("brushCount", this.brushCount);
    target.setFloat("refillRate", this.refillRate);
    target.setVector2("windDir", this.windDir);
  }

  private applyBrushesCpu(): void {
    for (let i = 0; i < this.brushCount; i++) {
      const base = i * 4;
      const wx = this.brushData[base]!;
      const wz = this.brushData[base + 1]!;
      const radius = Math.max(this.brushData[base + 2]!, 0.01);
      const r1 = MAX_BRUSHES * 4 + base;
      const depth = this.brushData[r1 + 2]!;
      const berm = this.brushData[r1 + 3]!;
      const r2 = MAX_BRUSHES * 8 + base;
      const compression = this.brushData[r2]!;
      const wet = this.brushData[r2 + 1]!;

      const minX = Math.max(
        0,
        Math.floor(((wx - radius - this.centerX) / this.extentM + 0.5) * this.resolution),
      );
      const maxX = Math.min(
        this.resolution - 1,
        Math.ceil(((wx + radius - this.centerX) / this.extentM + 0.5) * this.resolution),
      );
      const minZ = Math.max(
        0,
        Math.floor(((wz - radius - this.centerZ) / this.extentM + 0.5) * this.resolution),
      );
      const maxZ = Math.min(
        this.resolution - 1,
        Math.ceil(((wz + radius - this.centerZ) / this.extentM + 0.5) * this.resolution),
      );

      for (let z = minZ; z <= maxZ; z++) {
        for (let x = minX; x <= maxX; x++) {
          const worldX = this.centerX + ((x + 0.5) / this.resolution - 0.5) * this.extentM;
          const worldZ = this.centerZ + ((z + 0.5) / this.resolution - 0.5) * this.extentM;
          const t = Math.hypot(worldX - wx, worldZ - wz) / radius;
          if (t >= 1.35) continue;
          const core = 1 - smoothstep(0, 1, t);
          const floorW = smoothstep(0.15, 0.85, core);
          const idx = (z * this.resolution + x) * 4;
          this.cpuData[idx] = Math.min(0.65, this.cpuData[idx]! + depth * floorW);
          const ring = Math.exp(-(((t - 1.05) / 0.28) ** 2)) * (t >= 0.55 ? 1 : 0);
          this.cpuData[idx + 1] = Math.min(0.4, this.cpuData[idx + 1]! + berm * ring);
          this.cpuData[idx + 2] = Math.max(this.cpuData[idx + 2]!, compression * core);
          this.cpuData[idx + 3] = Math.max(this.cpuData[idx + 3]!, wet * core);
        }
      }
    }
  }

  private relaxCpu(dt: number): void {
    const k = Math.min(0.2, this.refillRate * dt * 50);
    if (k <= 0) return;
    for (let i = 0; i < this.cpuData.length; i += 4) {
      this.cpuData[i]! *= 1 - k * 0.4;
      this.cpuData[i + 1]! *= 1 - k * 0.5;
      this.cpuData[i + 2]! *= 1 - k * 0.2;
      this.cpuData[i + 3]! *= 1 - k * 0.15;
    }
  }

  dispose(): void {
    for (const t of this.targets) t.dispose();
    this.brushTex.dispose();
    this.cpuTex.dispose();
  }
}

function smoothstep(e0: number, e1: number, x: number): number {
  const t = Math.max(0, Math.min(1, (x - e0) / (e1 - e0)));
  return t * t * (3 - 2 * t);
}
