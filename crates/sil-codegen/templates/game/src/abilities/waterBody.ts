/**
 * Shared swept-spine water body — one mesh, multiple strand slots for spells.
 * Refraction samples the sky LUT (no scene copy).
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

export const STRAND_MAX = 8;
export const STRAND_COLS = 64;
const LATTICE_COLS = 96;
const RING = 16;

const WATER_VERT = `
attribute position : vec3<f32>;
uniform viewProjection : mat4x4<f32>;
uniform cameraPosition : vec3<f32>;
varying vWorldPos : vec3<f32>;
varying vNormal : vec3<f32>;
varying vViewDir : vec3<f32>;
varying vFoam : f32;
varying vStrand : f32;

var spineTex : texture_2d<f32>;
var spineTexSampler : sampler;

@vertex
fn main(input : VertexInputs) -> FragmentInputs {
  // position.x = col, position.y = ring, position.z = strand
  var col : f32 = vertexInputs.position.x;
  var ring : f32 = vertexInputs.position.y;
  var strand : f32 = vertexInputs.position.z;
  var cols : f32 = ${LATTICE_COLS}.0;
  var rings : f32 = ${RING}.0;
  var sampleCol : i32 = i32(clamp(col / cols * ${STRAND_COLS}.0, 0.0, ${STRAND_COLS - 1}.0));
  var si : i32 = i32(strand);
  var p0 : vec4<f32> = textureLoad(spineTex, vec2<i32>(sampleCol, si * 3), 0);
  var p1 : vec4<f32> = textureLoad(spineTex, vec2<i32>(min(sampleCol + 1, ${STRAND_COLS - 1}), si * 3), 0);
  var f0 : vec4<f32> = textureLoad(spineTex, vec2<i32>(sampleCol, si * 3 + 1), 0);
  var f1 : vec4<f32> = textureLoad(spineTex, vec2<i32>(min(sampleCol + 1, ${STRAND_COLS - 1}), si * 3 + 1), 0);
  var meta0 : vec4<f32> = textureLoad(spineTex, vec2<i32>(sampleCol, si * 3 + 2), 0);
  var t : f32 = fract(col / cols * ${STRAND_COLS}.0);
  var center : vec3<f32> = mix(p0.xyz, p1.xyz, t);
  var radius : f32 = mix(p0.w, p1.w, t);
  var right : vec3<f32> = normalize(mix(f0.xyz, f1.xyz, t) + vec3<f32>(0.001, 0.0, 0.0));
  var tangent : vec3<f32> = normalize(p1.xyz - p0.xyz + vec3<f32>(0.0, 0.0, 0.001));
  var up : vec3<f32> = normalize(cross(tangent, right));
  right = normalize(cross(up, tangent));
  var theta : f32 = ring / rings * 6.2831853;
  var local : vec3<f32> = (right * cos(theta) + up * sin(theta)) * radius;
  var wp : vec3<f32> = center + local;
  var n : vec3<f32> = normalize(local + vec3<f32>(0.0, 0.001, 0.0));
  vertexOutputs.vWorldPos = wp;
  vertexOutputs.vNormal = n;
  vertexOutputs.vViewDir = normalize(uniforms.cameraPosition - wp);
  vertexOutputs.vFoam = mix(meta0.x, meta0.x, t);
  vertexOutputs.vStrand = strand;
  vertexOutputs.position = uniforms.viewProjection * vec4<f32>(wp, 1.0);
}
`;

const WATER_FRAG = `
varying vWorldPos : vec3<f32>;
varying vNormal : vec3<f32>;
varying vViewDir : vec3<f32>;
varying vFoam : f32;
varying vStrand : f32;

uniform sunDir : vec3<f32>;
uniform sunColor : vec3<f32>;
uniform time : f32;

var skyLut : texture_2d<f32>;
var skyLutSampler : sampler;

@fragment
fn main(input : FragmentInputs) -> FragmentOutputs {
  var N : vec3<f32> = normalize(fragmentInputs.vNormal);
  var V : vec3<f32> = normalize(fragmentInputs.vViewDir);
  var L : vec3<f32> = normalize(uniforms.sunDir);
  // Animated flow normals
  var flow : f32 = sin(fragmentInputs.vWorldPos.x * 4.0 + uniforms.time * 3.0)
    * cos(fragmentInputs.vWorldPos.z * 3.5 + uniforms.time * 2.2) * 0.15;
  N = normalize(N + vec3<f32>(flow, 0.0, flow * 0.7));

  var fresnel : f32 = pow(1.0 - max(0.0, dot(N, V)), 3.0);
  var R : vec3<f32> = reflect(-V, N);
  var lutU : f32 = atan2(R.x, R.z) / 6.2831853 + 0.5;
  var lutV : f32 = 0.5 - asin(clamp(R.y, -1.0, 1.0)) / 3.14159265;
  var sky : vec3<f32> = textureSampleLevel(skyLut, skyLutSampler, vec2<f32>(lutU, lutV), 0.0).rgb;

  var absorb : vec3<f32> = vec3<f32>(0.05, 0.25, 0.35);
  var body : vec3<f32> = mix(absorb, sky, 0.55 + fresnel * 0.35);
  var spec : f32 = pow(max(0.0, dot(N, normalize(L + V))), 80.0);
  body = body + uniforms.sunColor * spec * 0.6;
  body = mix(body, vec3<f32>(0.92, 0.96, 1.0), fragmentInputs.vFoam * 0.7);
  // Chromatic hint
  body.r = body.r + fresnel * 0.04;
  body.b = body.b + fresnel * 0.06;
  var alpha : f32 = mix(0.55, 0.85, fresnel) + fragmentInputs.vFoam * 0.15;
  fragmentOutputs.color = vec4<f32>(body, clamp(alpha, 0.4, 0.92));
}
`;

export class WaterBody {
  readonly mesh: Mesh;
  private readonly spineData: Float32Array;
  private readonly spineTex: RawTexture;
  private readonly material: ShaderMaterial;
  private readonly active = new Uint8Array(STRAND_MAX);
  private time = 0;
  private readonly camPos = new Vector3();

  constructor(scene: Scene, skyLut: Texture | null, sunDir: Vector3) {
    this.spineData = new Float32Array(STRAND_COLS * STRAND_MAX * 3 * 4);
    this.spineTex = RawTexture.CreateRGBATexture(
      this.spineData,
      STRAND_COLS,
      STRAND_MAX * 3,
      scene,
      false,
      false,
      Constants.TEXTURE_NEAREST_SAMPLINGMODE,
      Constants.TEXTURETYPE_FLOAT,
    );

    ShaderStore.ShadersStoreWGSL["waterBodyVertexShader"] = WATER_VERT;
    ShaderStore.ShadersStoreWGSL["waterBodyFragmentShader"] = WATER_FRAG;

    const positions: number[] = [];
    const indices: number[] = [];
    for (let s = 0; s < STRAND_MAX; s++) {
      const base = s * (LATTICE_COLS + 1) * (RING + 1);
      for (let c = 0; c <= LATTICE_COLS; c++) {
        for (let r = 0; r <= RING; r++) {
          positions.push(c, r, s);
        }
      }
      for (let c = 0; c < LATTICE_COLS; c++) {
        for (let r = 0; r < RING; r++) {
          const i0 = base + c * (RING + 1) + r;
          const i1 = i0 + 1;
          const i2 = i0 + (RING + 1);
          const i3 = i2 + 1;
          indices.push(i0, i2, i1, i1, i2, i3);
        }
      }
    }

    this.mesh = new Mesh("waterBody", scene);
    const vd = new VertexData();
    vd.positions = positions;
    vd.indices = indices;
    vd.applyToMesh(this.mesh, false);
    this.mesh.alwaysSelectAsActiveMesh = true;
    this.mesh.alphaIndex = 10;

    this.material = new ShaderMaterial(
      "waterBodyMat",
      scene,
      { vertex: "waterBody", fragment: "waterBody" },
      {
        attributes: ["position"],
        uniforms: ["viewProjection", "cameraPosition", "sunDir", "sunColor", "time"],
        samplers: ["spineTex", "skyLut"],
        needAlphaBlending: true,
        shaderLanguage: ShaderLanguage.WGSL,
      },
    );
    this.material.setTexture("spineTex", this.spineTex);
    if (skyLut) this.material.setTexture("skyLut", skyLut);
    this.material.setVector3("sunDir", sunDir);
    this.material.setColor3("sunColor", new Color3(1, 0.9, 0.75));
    this.material.backFaceCulling = false;
    this.material.alphaMode = 2;
    this.mesh.material = this.material;
  }

  acquire(): number {
    for (let i = 0; i < STRAND_MAX; i++) {
      if (!this.active[i]) {
        this.active[i] = 1;
        return i;
      }
    }
    return -1;
  }

  release(strand: number): void {
    if (strand < 0 || strand >= STRAND_MAX) return;
    this.active[strand] = 0;
    const rowBase = strand * 3;
    for (let c = 0; c < STRAND_COLS; c++) {
      for (let row = 0; row < 3; row++) {
        const i = ((rowBase + row) * STRAND_COLS + c) * 4;
        this.spineData[i] = 0;
        this.spineData[i + 1] = 0;
        this.spineData[i + 2] = 0;
        this.spineData[i + 3] = 0;
      }
    }
  }

  hasActiveStrand(): boolean {
    for (let i = 0; i < STRAND_MAX; i++) {
      if (this.active[i]) return true;
    }
    return false;
  }

  /** Write spine column: center+radius, frame right, foam meta. */
  column(
    strand: number,
    col: number,
    x: number,
    y: number,
    z: number,
    radius: number,
    rx: number,
    ry: number,
    rz: number,
    foam: number,
  ): void {
    if (strand < 0 || col < 0 || col >= STRAND_COLS) return;
    const rowBase = strand * 3;
    let i = (rowBase * STRAND_COLS + col) * 4;
    this.spineData[i] = x;
    this.spineData[i + 1] = y;
    this.spineData[i + 2] = z;
    this.spineData[i + 3] = radius;
    i = ((rowBase + 1) * STRAND_COLS + col) * 4;
    this.spineData[i] = rx;
    this.spineData[i + 1] = ry;
    this.spineData[i + 2] = rz;
    this.spineData[i + 3] = 0;
    i = ((rowBase + 2) * STRAND_COLS + col) * 4;
    this.spineData[i] = foam;
    this.spineData[i + 1] = 0;
    this.spineData[i + 2] = 0;
    this.spineData[i + 3] = 0;
  }

  update(dt: number, scene: Scene): void {
    this.time += dt;
    this.spineTex.update(this.spineData);
    this.material.setFloat("time", this.time);
    const cam = scene.activeCamera;
    if (cam) {
      this.camPos.copyFrom(cam.globalPosition);
      this.material.setVector3("cameraPosition", this.camPos);
    }
  }

  dispose(): void {
    this.mesh.dispose();
    this.material.dispose();
    this.spineTex.dispose();
  }
}
