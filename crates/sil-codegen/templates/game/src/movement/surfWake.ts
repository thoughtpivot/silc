/**
 * Snow-surf wake — static lattice mesh driven by a spine data texture.
 */
import {
  Color3,
  Constants,
  Mesh,
  RawTexture,
  Scene,
  ShaderLanguage,
  ShaderMaterial,
  ShaderStore,
  Texture,
  Vector3,
  VertexData,
} from "@babylonjs/core";
import type { DeformationField } from "../terrain/deformation.ts";

const SPINE_MAX = 96;
const SPINE_STEP = 0.3;
const LIFE = 0.88;
const BOW_LEAD = 0.55;
const MAX_HEIGHT = 2.4;
const COLS = 96;
const ROWS = 14;

const WAKE_VERT = `
attribute position : vec3<f32>;
uniform viewProjection : mat4x4<f32>;
uniform cameraPosition : vec3<f32>;
uniform spineCount : f32;
varying vWorldPos : vec3<f32>;
varying vNormal : vec3<f32>;
varying vViewDir : vec3<f32>;
varying vAge : f32;

var spineTex : texture_2d<f32>;
var spineTexSampler : sampler;

@vertex
fn main(input : VertexInputs) -> FragmentInputs {
  var col : f32 = vertexInputs.position.x;
  var row : f32 = vertexInputs.position.y;
  var side : f32 = vertexInputs.position.z;
  var cols : f32 = ${COLS}.0;
  var rows : f32 = ${ROWS}.0;
  var count : f32 = max(uniforms.spineCount, 1.0);
  var si : f32 = (col / cols) * (count - 1.0);
  var i0 : i32 = i32(floor(si));
  var i1 : i32 = min(i0 + 1, i32(count) - 1);
  var t : f32 = fract(si);
  var a0 : vec4<f32> = textureLoad(spineTex, vec2<i32>(i0, 0), 0);
  var a1 : vec4<f32> = textureLoad(spineTex, vec2<i32>(i1, 0), 0);
  var b0 : vec4<f32> = textureLoad(spineTex, vec2<i32>(i0, 1), 0);
  var b1 : vec4<f32> = textureLoad(spineTex, vec2<i32>(i1, 1), 0);
  var center : vec3<f32> = mix(a0.xyz, a1.xyz, t);
  var right : vec3<f32> = normalize(mix(b0.xyz, b1.xyz, t));
  var age : f32 = mix(a0.w, a1.w, t);
  var height : f32 = mix(b0.w, b1.w, t);
  var across : f32 = row / rows;
  var wall : f32 = sin(across * 3.14159) * (1.0 - age);
  var curl : f32 = pow(across, 1.4) * (1.0 - age) * 0.55;
  var outward : f32 = (0.4 + across * 1.8) * side;
  var wp : vec3<f32> = center
    + right * outward
    + vec3<f32>(0.0, height * wall + curl * height * 0.5, 0.0);
  var n : vec3<f32> = normalize(vec3<f32>(side * 0.4, 0.85, 0.1) + right * side * 0.3);
  vertexOutputs.vWorldPos = wp;
  vertexOutputs.vNormal = n;
  vertexOutputs.vViewDir = normalize(uniforms.cameraPosition - wp);
  vertexOutputs.vAge = age;
  vertexOutputs.position = uniforms.viewProjection * vec4<f32>(wp, 1.0);
}
`;

const WAKE_FRAG = `
varying vWorldPos : vec3<f32>;
varying vNormal : vec3<f32>;
varying vViewDir : vec3<f32>;
varying vAge : f32;
uniform sunDir : vec3<f32>;
uniform sunColor : vec3<f32>;

@fragment
fn main(input : FragmentInputs) -> FragmentOutputs {
  var N : vec3<f32> = normalize(fragmentInputs.vNormal);
  var V : vec3<f32> = normalize(fragmentInputs.vViewDir);
  var L : vec3<f32> = normalize(uniforms.sunDir);
  var wrap : f32 = max(0.0, (dot(N, L) + 0.4) / 1.4);
  var albedo : vec3<f32> = mix(vec3<f32>(0.88, 0.93, 0.98), vec3<f32>(0.7, 0.8, 0.92), fragmentInputs.vAge);
  var spec : f32 = pow(max(0.0, dot(N, normalize(L + V))), 48.0);
  var col : vec3<f32> = albedo * (0.35 + wrap * 0.85) * uniforms.sunColor
    + uniforms.sunColor * spec * 0.5
    + vec3<f32>(0.55, 0.7, 0.95) * 0.15;
  var alpha : f32 = (1.0 - fragmentInputs.vAge) * 0.85;
  fragmentOutputs.color = vec4<f32>(col, alpha);
}
`;

export class SurfWake {
  readonly mesh: Mesh;
  private readonly x = new Float32Array(SPINE_MAX);
  private readonly y = new Float32Array(SPINE_MAX);
  private readonly z = new Float32Array(SPINE_MAX);
  private readonly age = new Float32Array(SPINE_MAX);
  private readonly hx = new Float32Array(SPINE_MAX);
  private readonly hz = new Float32Array(SPINE_MAX);
  private readonly heights = new Float32Array(SPINE_MAX);
  private head = 0;
  private count = 0;
  private distAcc = 0;
  private readonly spineData: Float32Array;
  private readonly spineTex: RawTexture;
  private readonly material: ShaderMaterial;
  private readonly camPos = new Vector3();
  private lastX = 0;
  private lastZ = 0;
  private sprayBurst = 0;

  constructor(scene: Scene, sunDir: Vector3) {
    this.spineData = new Float32Array(SPINE_MAX * 2 * 4);
    this.spineTex = RawTexture.CreateRGBATexture(
      this.spineData,
      SPINE_MAX,
      2,
      scene,
      false,
      false,
      Constants.TEXTURE_NEAREST_SAMPLINGMODE,
      Constants.TEXTURETYPE_FLOAT,
    );

    ShaderStore.ShadersStoreWGSL["surfWakeVertexShader"] = WAKE_VERT;
    ShaderStore.ShadersStoreWGSL["surfWakeFragmentShader"] = WAKE_FRAG;

    const positions: number[] = [];
    const indices: number[] = [];
    for (const side of [-1, 1]) {
      const base = side < 0 ? 0 : (COLS + 1) * (ROWS + 1);
      for (let c = 0; c <= COLS; c++) {
        for (let r = 0; r <= ROWS; r++) {
          positions.push(c, r, side);
        }
      }
      for (let c = 0; c < COLS; c++) {
        for (let r = 0; r < ROWS; r++) {
          const i0 = base + c * (ROWS + 1) + r;
          const i1 = i0 + 1;
          const i2 = i0 + (ROWS + 1);
          const i3 = i2 + 1;
          indices.push(i0, i2, i1, i1, i2, i3);
        }
      }
    }

    this.mesh = new Mesh("surfWake", scene);
    const vd = new VertexData();
    vd.positions = positions;
    vd.indices = indices;
    vd.applyToMesh(this.mesh, false);
    this.mesh.alwaysSelectAsActiveMesh = true;
    this.mesh.alphaIndex = 5;

    this.material = new ShaderMaterial(
      "surfWakeMat",
      scene,
      { vertex: "surfWake", fragment: "surfWake" },
      {
        attributes: ["position"],
        uniforms: ["viewProjection", "cameraPosition", "spineCount", "sunDir", "sunColor"],
        samplers: ["spineTex"],
        needAlphaBlending: true,
        shaderLanguage: ShaderLanguage.WGSL,
      },
    );
    this.material.setTexture("spineTex", this.spineTex);
    this.material.setVector3("sunDir", sunDir);
    this.material.setColor3("sunColor", new Color3(1, 0.9, 0.75));
    this.material.backFaceCulling = false;
    this.mesh.material = this.material;
    this.mesh.isVisible = false;
  }

  reset(): void {
    this.count = 0;
    this.head = 0;
    this.distAcc = 0;
    this.mesh.isVisible = false;
  }

  update(
    dt: number,
    active: boolean,
    px: number,
    py: number,
    pz: number,
    yaw: number,
    speed: number,
    turnRate: number,
    deformation: DeformationField,
    scene: Scene,
  ): void {
    if (!active) {
      // Age out existing spine
      for (let i = 0; i < this.count; i++) {
        const idx = (this.head - this.count + i + SPINE_MAX * 2) % SPINE_MAX;
        this.age[idx]! += dt / LIFE;
      }
      this.upload(scene);
      if (this.count === 0) this.mesh.isVisible = false;
      return;
    }

    this.mesh.isVisible = true;
    const fwdX = Math.sin(yaw);
    const fwdZ = Math.cos(yaw);
    const rightX = Math.cos(yaw);
    const rightZ = -Math.sin(yaw);
    const bowX = px + fwdX * BOW_LEAD;
    const bowZ = pz + fwdZ * BOW_LEAD;

    const dx = bowX - this.lastX;
    const dz = bowZ - this.lastZ;
    this.distAcc += Math.hypot(dx, dz);
    this.lastX = bowX;
    this.lastZ = bowZ;

    while (this.distAcc >= SPINE_STEP) {
      this.distAcc -= SPINE_STEP;
      const idx = this.head % SPINE_MAX;
      this.x[idx] = bowX;
      this.y[idx] = py;
      this.z[idx] = bowZ;
      this.age[idx] = 0;
      this.hx[idx] = rightX;
      this.hz[idx] = rightZ;
      const carve = Math.min(1, Math.abs(turnRate) * 2.5 + speed / 14);
      this.heights[idx] = MAX_HEIGHT * carve * (0.55 + speed / 28);
      this.head = (this.head + 1) % SPINE_MAX;
      this.count = Math.min(SPINE_MAX, this.count + 1);
    }

    for (let i = 0; i < this.count; i++) {
      const idx = (this.head - this.count + i + SPINE_MAX * 2) % SPINE_MAX;
      this.age[idx]! += dt / LIFE;
    }

    // Drop aged samples from the tail
    while (this.count > 0) {
      const tail = (this.head - this.count + SPINE_MAX * 2) % SPINE_MAX;
      if (this.age[tail]! < 1) break;
      this.count--;
    }

    // Carve terrain
    const depth = 0.28 + (speed / 14) * 0.2;
    deformation.brush(px, pz, 1.1 + speed * 0.04, depth, {
      shape: "groove",
      yaw,
      compression: 0.7,
      berm: depth * 0.7,
      elongation: 2.4,
    });

    this.sprayBurst += dt;
    this.upload(scene);
  }

  private upload(scene: Scene): void {
    for (let i = 0; i < this.count; i++) {
      const idx = (this.head - this.count + i + SPINE_MAX * 2) % SPINE_MAX;
      const o = i * 4;
      this.spineData[o] = this.x[idx]!;
      this.spineData[o + 1] = this.y[idx]!;
      this.spineData[o + 2] = this.z[idx]!;
      this.spineData[o + 3] = this.age[idx]!;
      const o1 = SPINE_MAX * 4 + i * 4;
      this.spineData[o1] = this.hx[idx]!;
      this.spineData[o1 + 1] = 0;
      this.spineData[o1 + 2] = this.hz[idx]!;
      this.spineData[o1 + 3] = this.heights[idx]!;
    }
    this.spineTex.update(this.spineData);
    this.material.setFloat("spineCount", Math.max(this.count, 1));
    const cam = scene.activeCamera;
    if (cam) {
      this.camPos.copyFrom(cam.globalPosition);
      this.material.setVector3("cameraPosition", this.camPos);
    }
  }

  getSprayReady(): boolean {
    if (this.sprayBurst > 0.05) {
      this.sprayBurst = 0;
      return true;
    }
    return false;
  }

  dispose(): void {
    this.mesh.dispose();
    this.material.dispose();
    this.spineTex.dispose();
  }
}
