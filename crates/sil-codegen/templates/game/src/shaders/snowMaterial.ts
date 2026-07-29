/**
 * Snow material — VS places clipmap verts from height+deform textures;
 * FS does multi-scale normals, SSS, glint, fog, spell light.
 */
import {
  Color3,
  Scene,
  ShaderLanguage,
  ShaderMaterial,
  ShaderStore,
  Vector2,
  Vector3,
} from "@babylonjs/core";
import type { GameManifest } from "../manifest.ts";
import type { DeformationField } from "../terrain/deformation.ts";
import type { Heightfield } from "../terrain/heightfield.ts";
import { BASE_SPACING, GRID_N } from "../terrain/clipmapMesh.ts";

const VERT = `
attribute position : vec3<f32>;

uniform viewProjection : mat4x4<f32>;
uniform cameraPosition : vec3<f32>;
uniform lodCenter : vec3<f32>;
uniform baseSpacing : f32;
uniform gridHalfN : f32;
uniform worldOrigin : vec2<f32>;
uniform worldSize : f32;
uniform heightRes : f32;
uniform deformationExtent : f32;
uniform deformationCenter : vec3<f32>;
uniform deformDepthScale : f32;

var heightSampler : texture_2d<f32>;
var heightSamplerSampler : sampler;
var deformationSampler : texture_2d<f32>;
var deformationSamplerSampler : sampler;

varying vWorldPos : vec3<f32>;
varying vViewDir : vec3<f32>;
varying vSpacing : f32;
varying vHeightUV : vec2<f32>;

fn sampleHeightBicubic(uv : vec2<f32>, res : f32) -> f32 {
  var coord : vec2<f32> = uv * res - 0.5;
  var base : vec2<f32> = floor(coord);
  var f : vec2<f32> = coord - base;
  var f2 : vec2<f32> = f * f;
  var f3 : vec2<f32> = f2 * f;
  var w0 : vec2<f32> = (1.0 - 3.0 * f + 3.0 * f2 - f3) / 6.0;
  var w1 : vec2<f32> = (4.0 - 6.0 * f2 + 3.0 * f3) / 6.0;
  var w2 : vec2<f32> = (1.0 + 3.0 * f + 3.0 * f2 - 3.0 * f3) / 6.0;
  var w3 : vec2<f32> = f3 / 6.0;
  var s0 : vec2<f32> = w0 + w1;
  var s1 : vec2<f32> = w2 + w3;
  var o0 : vec2<f32> = (base + 0.5 - 1.0 + w1 / s0) / res;
  var o1 : vec2<f32> = (base + 0.5 + 1.0 + w3 / s1) / res;
  var t00 : f32 = textureSampleLevel(heightSampler, heightSamplerSampler, vec2<f32>(o0.x, o0.y), 0.0).r;
  var t10 : f32 = textureSampleLevel(heightSampler, heightSamplerSampler, vec2<f32>(o1.x, o0.y), 0.0).r;
  var t01 : f32 = textureSampleLevel(heightSampler, heightSamplerSampler, vec2<f32>(o0.x, o1.y), 0.0).r;
  var t11 : f32 = textureSampleLevel(heightSampler, heightSamplerSampler, vec2<f32>(o1.x, o1.y), 0.0).r;
  return mix(mix(t00, t10, s1.x), mix(t01, t11, s1.x), s1.y);
}

fn placeClipmap(grid : vec2<f32>, level : f32, camXZ : vec2<f32>) -> vec3<f32> {
  var spacing : f32 = uniforms.baseSpacing * exp2(level);
  var snap : f32 = spacing * 2.0;
  var origin : vec2<f32> = floor(camXZ / snap + 0.5) * snap;
  var local : vec2<f32> = grid * spacing;
  var extent : f32 = uniforms.gridHalfN * spacing;
  var cheb : f32 = max(abs(local.x), abs(local.y)) / max(extent, 0.001);
  var morph : f32 = clamp((cheb - 0.70) / 0.16, 0.0, 1.0);
  var coarse : vec2<f32> = floor(grid * 0.5) * 2.0;
  var g : vec2<f32> = mix(grid, coarse, morph);
  local = g * spacing;
  spacing = spacing * (1.0 + morph);
  var worldXZ : vec2<f32> = origin + local;
  return vec3<f32>(worldXZ.x, spacing, worldXZ.y);
}

fn sampleDeform(worldXZ : vec2<f32>) -> vec4<f32> {
  var uv : vec2<f32> = fract(worldXZ / uniforms.deformationExtent);
  return textureSampleLevel(deformationSampler, deformationSamplerSampler, uv, 0.0);
}

@vertex
fn main(input : VertexInputs) -> FragmentInputs {
  var grid : vec2<f32> = vec2<f32>(vertexInputs.position.x, vertexInputs.position.z);
  var level : f32 = vertexInputs.position.y;
  var placed : vec3<f32> = placeClipmap(grid, level, uniforms.lodCenter.xz);
  var worldXZ : vec2<f32> = vec2<f32>(placed.x, placed.z);
  var spacing : f32 = placed.y;
  var huv : vec2<f32> = (worldXZ - uniforms.worldOrigin) / uniforms.worldSize;
  var h : f32 = sampleHeightBicubic(clamp(huv, vec2<f32>(0.001), vec2<f32>(0.999)), uniforms.heightRes);
  var defH : f32 = 0.0;
  if (spacing < 1.0) {
    var d : vec4<f32> = sampleDeform(worldXZ);
    defH = (d.y - d.x) * uniforms.deformDepthScale;
    var fade : f32 = 1.0 - smoothstep(0.45, 1.0, spacing);
    defH = defH * fade;
  }
  var wp : vec3<f32> = vec3<f32>(worldXZ.x, h + defH, worldXZ.y);
  vertexOutputs.vWorldPos = wp;
  vertexOutputs.vViewDir = normalize(uniforms.cameraPosition - wp);
  vertexOutputs.vSpacing = spacing;
  vertexOutputs.vHeightUV = huv;
  vertexOutputs.position = uniforms.viewProjection * vec4<f32>(wp, 1.0);
}
`;

const FRAG = `
varying vWorldPos : vec3<f32>;
varying vViewDir : vec3<f32>;
varying vSpacing : f32;
varying vHeightUV : vec2<f32>;

uniform deformationExtent : f32;
uniform deformationCenter : vec3<f32>;
uniform worldOrigin : vec2<f32>;
uniform worldSize : f32;
uniform heightRes : f32;
uniform sunDir : vec3<f32>;
uniform sunColor : vec3<f32>;
uniform ambientColor : vec3<f32>;
uniform sh0 : vec3<f32>;
uniform sh1 : vec3<f32>;
uniform sh2 : vec3<f32>;
uniform glintIntensity : f32;
uniform scatterStrength : f32;
uniform fogDensity : f32;
uniform fogColor : vec3<f32>;
uniform spellLightPos : vec3<f32>;
uniform spellLightColor : vec3<f32>;
uniform spellLightIntensity : f32;
uniform spellLightRange : f32;
uniform skyTint : vec3<f32>;

var heightSampler : texture_2d<f32>;
var heightSamplerSampler : sampler;
var deformationSampler : texture_2d<f32>;
var deformationSamplerSampler : sampler;

fn hash(p : vec2<f32>) -> f32 {
  return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn noise(p : vec2<f32>) -> f32 {
  var i : vec2<f32> = floor(p);
  var f : vec2<f32> = fract(p);
  f = f * f * (3.0 - 2.0 * f);
  var a : f32 = hash(i);
  var b : f32 = hash(i + vec2<f32>(1.0, 0.0));
  var c : f32 = hash(i + vec2<f32>(0.0, 1.0));
  var d : f32 = hash(i + vec2<f32>(1.0, 1.0));
  return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

fn sampleDeform(worldXZ : vec2<f32>) -> vec4<f32> {
  var uv : vec2<f32> = fract(worldXZ / uniforms.deformationExtent);
  var halfE : f32 = uniforms.deformationExtent * 0.5;
  var d : vec2<f32> = abs(worldXZ - uniforms.deformationCenter.xz);
  var fall : f32 = 1.0 - smoothstep(halfE * 0.80, halfE * 0.96, max(d.x, d.y));
  var s : vec4<f32> = textureSampleLevel(deformationSampler, deformationSamplerSampler, uv, 0.0);
  return s * fall;
}

fn heightGrad(uv : vec2<f32>, eps : f32) -> vec2<f32> {
  var hx : f32 = textureSampleLevel(heightSampler, heightSamplerSampler, uv + vec2<f32>(eps, 0.0), 0.0).r
    - textureSampleLevel(heightSampler, heightSamplerSampler, uv - vec2<f32>(eps, 0.0), 0.0).r;
  var hz : f32 = textureSampleLevel(heightSampler, heightSamplerSampler, uv + vec2<f32>(0.0, eps), 0.0).r
    - textureSampleLevel(heightSampler, heightSamplerSampler, uv - vec2<f32>(0.0, eps), 0.0).r;
  return vec2<f32>(hx / (2.0 * eps * uniforms.worldSize), hz / (2.0 * eps * uniforms.worldSize));
}

fn triplanarDetail(n : vec3<f32>, p : vec3<f32>, scale : f32, strength : f32) -> vec3<f32> {
  var blend : vec3<f32> = abs(n);
  blend = pow(blend, vec3<f32>(4.0));
  blend = blend / (blend.x + blend.y + blend.z + 0.001);
  var py : vec2<f32> = p.xz * scale;
  var ny : vec3<f32> = vec3<f32>(noise(py) - 0.5, 0.0, noise(py + 31.0) - 0.5) * strength;
  return normalize(n + ny);
}

fn evalSH(n : vec3<f32>) -> vec3<f32> {
  return uniforms.sh0
    + uniforms.sh1 * n.y
    + uniforms.sh2 * (n.x * 0.5 + n.z * 0.5);
}

@fragment
fn main(input : FragmentInputs) -> FragmentOutputs {
  var V : vec3<f32> = normalize(fragmentInputs.vViewDir);
  var L : vec3<f32> = normalize(uniforms.sunDir);
  var p : vec3<f32> = fragmentInputs.vWorldPos;
  var huv : vec2<f32> = fragmentInputs.vHeightUV;
  var eps : f32 = max(1.5 / uniforms.heightRes, 0.0004);
  var g : vec2<f32> = heightGrad(huv, eps);

  var state : vec4<f32> = sampleDeform(p.xz);
  var depression : f32 = state.x;
  var displaced : f32 = state.y;
  var compression : f32 = state.z;
  var wetIce : f32 = state.w;
  var wetness : f32 = clamp(wetIce * 0.65, 0.0, 1.0);
  var ice : f32 = clamp(wetIce * 1.1 - 0.15, 0.0, 1.0);

  var defEps : f32 = max(0.08, fragmentInputs.vSpacing * 1.4);
  var dL : f32 = sampleDeform(p.xz + vec2<f32>(-defEps, 0.0)).y - sampleDeform(p.xz + vec2<f32>(-defEps, 0.0)).x;
  var dR : f32 = sampleDeform(p.xz + vec2<f32>(defEps, 0.0)).y - sampleDeform(p.xz + vec2<f32>(defEps, 0.0)).x;
  var dD : f32 = sampleDeform(p.xz + vec2<f32>(0.0, -defEps)).y - sampleDeform(p.xz + vec2<f32>(0.0, -defEps)).x;
  var dU : f32 = sampleDeform(p.xz + vec2<f32>(0.0, defEps)).y - sampleDeform(p.xz + vec2<f32>(0.0, defEps)).x;
  g = g + vec2<f32>((dL - dR) / (2.0 * defEps), (dD - dU) / (2.0 * defEps));

  var N : vec3<f32> = normalize(vec3<f32>(-g.x, 1.0, -g.y));
  var n1 : vec3<f32> = triplanarDetail(N, p, 0.13, 0.55);
  var n2 : vec3<f32> = triplanarDetail(N, p, 0.55, 0.4);
  var n3 : vec3<f32> = triplanarDetail(N, p, 2.8, 0.28);
  var dist : f32 = length(p.xz - uniforms.deformationCenter.xz);
  var blendMid : f32 = smoothstep(6.0, 28.0, dist);
  var blendFar : f32 = smoothstep(30.0, 100.0, dist);
  var Ndet : vec3<f32> = normalize(mix(mix(n3, n2, blendMid), n1, blendFar));

  var slope : f32 = 1.0 - abs(Ndet.y);
  var albedo : vec3<f32> = vec3<f32>(0.93, 0.95, 0.99);
  albedo = mix(albedo, vec3<f32>(0.68, 0.76, 0.88), smoothstep(0.04, 0.28, slope));
  albedo = mix(albedo, vec3<f32>(0.62, 0.66, 0.74), compression * 0.7);
  albedo = mix(albedo, vec3<f32>(0.9, 0.93, 0.98), displaced * 0.3);
  albedo = mix(albedo, vec3<f32>(0.55, 0.68, 0.82), wetness * 0.55);
  albedo = mix(albedo, vec3<f32>(0.78, 0.88, 0.96), ice * 0.45);
  albedo = albedo - depression * 0.16;

  var NdotL : f32 = dot(Ndet, L);
  var wrap : f32 = max(0.0, (NdotL + 0.35) / 1.35);
  var wrap2 : f32 = max(0.0, (NdotL + 0.75) / 1.75);
  var thickness : f32 = clamp(1.0 - compression * 0.55 - ice * 0.2, 0.25, 1.0);
  var backScatter : f32 = pow(max(0.0, dot(-V, L)), 1.55) * uniforms.scatterStrength * thickness;
  var sideScatter : f32 = pow(1.0 - abs(NdotL), 2.0) * uniforms.scatterStrength * 0.4 * thickness;
  var ambient : vec3<f32> = evalSH(Ndet) * 0.55 + uniforms.ambientColor * 0.45;
  var diffuse : vec3<f32> = albedo * (uniforms.sunColor * (wrap * 1.15 + wrap2 * 0.3) + ambient * 0.85);
  diffuse = diffuse + vec3<f32>(0.7, 0.84, 1.0) * backScatter * 1.25;
  diffuse = diffuse + vec3<f32>(0.25, 0.4, 0.65) * sideScatter;
  diffuse = mix(diffuse, diffuse * vec3<f32>(0.5, 0.64, 0.88), smoothstep(0.04, 0.4, depression));

  var H : vec3<f32> = normalize(L + V);
  var rough : f32 = mix(0.32, 0.1, ice);
  rough = mix(rough, 0.55, compression);
  rough = mix(rough, 0.16, wetness * 0.5);
  var specPower : f32 = mix(20.0, 160.0, 1.0 - rough);
  var fresnel : f32 = pow(1.0 - max(0.0, dot(Ndet, V)), 4.0);
  var spec : f32 = pow(max(0.0, dot(Ndet, H)), specPower) * (0.14 + fresnel * 0.4 + ice * 0.22);

  var sparkleHash : f32 = hash(floor(p.xz * 140.0));
  var sparkleHash2 : f32 = hash(floor(p.xz * 380.0 + 11.0));
  var grazing : f32 = pow(1.0 - max(0.0, dot(Ndet, V)), 3.0);
  var glint : f32 = step(0.88, sparkleHash) * grazing * uniforms.glintIntensity * (1.0 - compression);
  glint = glint + step(0.93, sparkleHash2) * grazing * uniforms.glintIntensity * 0.75;
  spec = spec + glint * 3.8;

  var color : vec3<f32> = diffuse + uniforms.sunColor * spec;
  var grain : f32 = noise(p.xz * 2.2) * 0.05 + noise(p.xz * 14.0) * 0.03 + noise(p.xz * 55.0) * 0.018;
  color = color * (0.93 + grain);
  color = mix(color, color * 0.7 + vec3<f32>(0.16, 0.3, 0.5), slope * 0.28);

  if (uniforms.spellLightIntensity > 0.01) {
    var toL : vec3<f32> = uniforms.spellLightPos - p;
    var ld : f32 = length(toL);
    var atten : f32 = clamp(1.0 - ld / max(uniforms.spellLightRange, 0.1), 0.0, 1.0);
    atten = atten * atten;
    var Ls : vec3<f32> = toL / max(ld, 0.001);
    var wrapS : f32 = max(0.0, (dot(Ndet, Ls) + 0.5) / 1.5);
    var sss : f32 = pow(max(0.0, dot(-V, Ls)), 1.8) * 0.45 * thickness;
    color = color + uniforms.spellLightColor * uniforms.spellLightIntensity * atten * (wrapS * 0.55 + sss);
    color = color + uniforms.spellLightColor * glint * atten * 0.8;
  }

  var fogAmt : f32 = 1.0 - exp(-dist * uniforms.fogDensity);
  fogAmt = clamp(fogAmt + smoothstep(80.0, 320.0, dist) * 0.22, 0.0, 0.72);
  color = mix(color, uniforms.fogColor * uniforms.skyTint, fogAmt);
  color = max(color, vec3<f32>(0.0));
  color = color * (1.15 / (1.0 + max(color.r, max(color.g, color.b)) * 0.35));
  fragmentOutputs.color = vec4<f32>(color, 1.0);
}
`;

export type SnowMaterialHandle = {
  material: ShaderMaterial;
  update: (deformation: DeformationField, lodX: number, lodZ: number) => void;
  setSpellLight: (pos: Vector3, color: Color3, intensity: number, range: number) => void;
  setSkySH: (sh0: Color3, sh1: Color3, sh2: Color3, tint: Color3) => void;
  dispose: () => void;
};

export function createSnowMaterial(
  scene: Scene,
  deformation: DeformationField,
  heightfield: Heightfield,
  manifest: GameManifest,
): SnowMaterialHandle {
  ShaderStore.ShadersStoreWGSL["snowVertexShader"] = VERT;
  ShaderStore.ShadersStoreWGSL["snowFragmentShader"] = FRAG;

  const glint = manifest.surface?.glint ?? 0.35;
  const scatter = manifest.surface?.scatter ?? 0.55;
  const fogDensity = manifest.environment?.fogDensity ?? 0.0008;
  const sunElev = ((manifest.environment?.sunElevationDeg ?? 12) * Math.PI) / 180;
  const sunAz = ((manifest.environment?.sunAzimuthDeg ?? 35) * Math.PI) / 180;
  const sunDir = new Vector3(
    Math.cos(sunElev) * Math.sin(sunAz),
    Math.sin(sunElev),
    Math.cos(sunElev) * Math.cos(sunAz),
  );

  const material = new ShaderMaterial(
    "snowMaterial",
    scene,
    { vertex: "snow", fragment: "snow" },
    {
      attributes: ["position"],
      uniforms: [
        "viewProjection",
        "cameraPosition",
        "lodCenter",
        "baseSpacing",
        "gridHalfN",
        "worldOrigin",
        "worldSize",
        "heightRes",
        "deformationExtent",
        "deformationCenter",
        "deformDepthScale",
        "sunDir",
        "sunColor",
        "ambientColor",
        "sh0",
        "sh1",
        "sh2",
        "glintIntensity",
        "scatterStrength",
        "fogDensity",
        "fogColor",
        "spellLightPos",
        "spellLightColor",
        "spellLightIntensity",
        "spellLightRange",
        "skyTint",
      ],
      samplers: ["heightSampler", "deformationSampler"],
      shaderLanguage: ShaderLanguage.WGSL,
    },
  );

  material.setTexture("heightSampler", heightfield.heightTex);
  material.setFloat("baseSpacing", BASE_SPACING);
  material.setFloat("gridHalfN", GRID_N / 2);
  material.setVector2("worldOrigin", heightfield.origin);
  material.setFloat("worldSize", heightfield.size);
  material.setFloat("heightRes", heightfield.heightTex.getSize().width);
  material.setFloat("deformDepthScale", 1.0);
  material.setVector3("sunDir", sunDir);
  material.setColor3("sunColor", new Color3(1.0, 0.88, 0.72));
  material.setColor3("ambientColor", new Color3(0.28, 0.36, 0.52));
  material.setColor3("sh0", new Color3(0.32, 0.4, 0.55));
  material.setColor3("sh1", new Color3(0.1, 0.14, 0.22));
  material.setColor3("sh2", new Color3(0.05, 0.06, 0.1));
  material.setColor3("skyTint", new Color3(1, 1, 1));
  material.setFloat("glintIntensity", Math.max(glint, 0.45));
  material.setFloat("scatterStrength", Math.max(scatter, 0.95));
  // Authored fogDensity can white-out the mid-field — keep horizon/sky readable.
  material.setFloat("fogDensity", Math.min(Math.max(fogDensity, 0.00015), 0.00055));
  material.setColor3("fogColor", new Color3(0.55, 0.68, 0.88));
  material.setVector3("spellLightPos", Vector3.Zero());
  material.setColor3("spellLightColor", new Color3(1, 0.9, 0.75));
  material.setFloat("spellLightIntensity", 0);
  material.setFloat("spellLightRange", 6);
  material.backFaceCulling = false;
  material.forceDepthWrite = true;
  material.checkReadyOnEveryCall = true;

  const camPos = new Vector3();
  const lodCenter = new Vector3();
  const defCenter = new Vector3();

  const update = (def: DeformationField, lodX: number, lodZ: number): void => {
    material.setTexture("deformationSampler", def.texture);
    material.setFloat("deformationExtent", def.extentM);
    defCenter.set(def.centerX, 0, def.centerZ);
    material.setVector3("deformationCenter", defCenter);
    lodCenter.set(lodX, 0, lodZ);
    material.setVector3("lodCenter", lodCenter);
    const cam = scene.activeCamera;
    if (cam) {
      camPos.copyFrom(cam.globalPosition);
      material.setVector3("cameraPosition", camPos);
    }
  };

  const setSpellLight = (pos: Vector3, color: Color3, intensity: number, range: number): void => {
    material.setVector3("spellLightPos", pos);
    material.setColor3("spellLightColor", color);
    material.setFloat("spellLightIntensity", intensity);
    material.setFloat("spellLightRange", range);
  };

  const setSkySH = (sh0: Color3, sh1: Color3, sh2: Color3, tint: Color3): void => {
    material.setColor3("sh0", sh0);
    material.setColor3("sh1", sh1);
    material.setColor3("sh2", sh2);
    material.setColor3("skyTint", tint);
  };

  update(deformation, 0, 0);

  return {
    material,
    update,
    setSpellLight,
    setSkySH,
    dispose: () => material.dispose(),
  };
}
