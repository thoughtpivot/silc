import {
  Color3,
  Color4,
  DirectionalLight,
  HemisphericLight,
  Mesh,
  MeshBuilder,
  ParticleSystem,
  RawTexture,
  Scene,
  ShaderLanguage,
  ShaderMaterial,
  ShaderStore,
  StandardMaterial,
  Texture,
  Vector3,
} from "@babylonjs/core";
import type { GameManifest } from "../manifest.ts";
import { whenReady } from "../core/gpuUtil.ts";

export type AtmosphereHandle = {
  sun: DirectionalLight;
  /** Reserved for custom cascade depth matching VS clipmap. */
  shadow: null;
  sunDir: Vector3;
  sh0: Color3;
  sh1: Color3;
  sh2: Color3;
  skyTint: Color3;
  skyLut: RawTexture | null;
  update: (dt: number, followX?: number, followZ?: number) => void;
  dispose: () => void;
};

export function addShadowCaster(_shadow: null, _mesh: Mesh): void {
  // No-op until custom cascade shadows land.
}

const SKY_VERT = `
attribute position : vec3<f32>;
uniform viewProjection : mat4x4<f32>;
varying vDir : vec3<f32>;
@vertex
fn main(input : VertexInputs) -> FragmentInputs {
  vertexOutputs.vDir = normalize(vertexInputs.position);
  var pos : vec4<f32> = uniforms.viewProjection * vec4<f32>(vertexInputs.position, 1.0);
  pos.z = pos.w * 0.999;
  vertexOutputs.position = pos;
}
`;

const SKY_FRAG = `
varying vDir : vec3<f32>;
uniform sunDir : vec3<f32>;
uniform sunColor : vec3<f32>;
uniform groundBounce : vec3<f32>;
uniform fogColor : vec3<f32>;

fn rayleighPhase(mu : f32) -> f32 {
  return 0.059683 * (1.0 + mu * mu);
}

fn miePhase(mu : f32, g : f32) -> f32 {
  var g2 : f32 = g * g;
  return 0.119366 * (1.0 - g2) / pow(1.0 + g2 - 2.0 * g * mu, 1.5);
}

@fragment
fn main(input : FragmentInputs) -> FragmentOutputs {
  var dir : vec3<f32> = normalize(fragmentInputs.vDir);
  var up : f32 = max(dir.y, 0.0);
  var mu : f32 = dot(dir, normalize(uniforms.sunDir));
  var t : f32 = pow(1.0 - up, 2.5);

  // Simplified atmospheric scatter — keep a readable blue even at low sun.
  var rayleigh : vec3<f32> = vec3<f32>(0.32, 0.52, 0.95) * rayleighPhase(mu) * (0.45 + up * 1.55);
  var mie : vec3<f32> = uniforms.sunColor * miePhase(mu, 0.76) * 0.55;
  var horizon : vec3<f32> = mix(vec3<f32>(0.62, 0.74, 0.92), uniforms.fogColor, 0.25);
  var col : vec3<f32> = mix(horizon, rayleigh + mie * 0.7, clamp(up * 1.15, 0.0, 1.0));
  col = col + uniforms.groundBounce * pow(1.0 - up, 3.0) * 0.18;

  // Solar disc
  var sunDisc : f32 = smoothstep(0.9992, 0.9998, mu);
  col = col + uniforms.sunColor * sunDisc * 8.0;

  // Far mountain silhouettes (raymarched noise ridges)
  if (dir.y > -0.02 && dir.y < 0.18) {
    var ang : f32 = atan2(dir.x, dir.z);
    var ridge : f32 = 0.04
      + 0.03 * sin(ang * 3.0 + 1.2)
      + 0.02 * sin(ang * 7.0)
      + 0.015 * sin(ang * 15.0 + 0.4);
    var sil : f32 = smoothstep(ridge + 0.01, ridge - 0.005, dir.y);
    col = mix(col, vec3<f32>(0.22, 0.28, 0.38) * (0.6 + up), sil * 0.85);
  }

  fragmentOutputs.color = vec4<f32>(col, 1.0);
}
`;

export function setupAtmosphere(scene: Scene, manifest: GameManifest): AtmosphereHandle {
  const fogDensity = manifest.environment?.fogDensity ?? 0.0008;
  // Stock Babylon fog blacks ShaderMaterial-only frames when density is off;
  // snow/sky shaders carry their own atmospheric fade.
  scene.fogMode = Scene.FOGMODE_NONE;
  scene.fogColor = new Color3(0.55, 0.68, 0.85);
  scene.clearColor = new Color4(0.45, 0.58, 0.78, 1);

  const sunElev = manifest.environment?.sunElevationDeg ?? 12;
  const sunAz = manifest.environment?.sunAzimuthDeg ?? 35;
  const elevRad = (sunElev * Math.PI) / 180;
  const azRad = (sunAz * Math.PI) / 180;
  const sunDir = new Vector3(
    Math.cos(elevRad) * Math.sin(azRad),
    Math.sin(elevRad),
    Math.cos(elevRad) * Math.cos(azRad),
  );

  const hemi = new HemisphericLight("ambient", new Vector3(0, 1, 0), scene);
  hemi.intensity = 0.55;
  hemi.diffuse = new Color3(0.42, 0.56, 0.82);
  hemi.groundColor = new Color3(0.55, 0.62, 0.72);

  const sun = new DirectionalLight("sun", sunDir.scale(-1), scene);
  sun.intensity = sunElev < 16 ? 1.2 : 1.4;
  sun.diffuse = new Color3(1.0, 0.84, 0.64);
  sun.specular = new Color3(0.85, 0.78, 0.65);
  sun.shadowEnabled = false;

  const sh0 = new Color3(0.28, 0.38, 0.55);
  const sh1 = new Color3(0.12, 0.16, 0.28);
  const sh2 = new Color3(0.05, 0.06, 0.1);
  // Snow ground bounce into SH
  const bounce = new Color3(0.83, 0.86, 0.91);
  sh0.r = sh0.r * 0.7 + bounce.r * 0.18;
  sh0.g = sh0.g * 0.7 + bounce.g * 0.18;
  sh0.b = sh0.b * 0.7 + bounce.b * 0.22;

  const skyTint = new Color3(1, 1, 1);
  buildSkyDome(scene, sunDir, scene.fogColor);
  const skyLut = bakeSkyLut(scene, sunDir);

  let spindrift: ParticleSystem | null = null;
  const spindriftEmitter = new Vector3();
  if (manifest.environment?.spindrift !== false) {
    spindrift = buildSpindrift(scene);
  }

  let t = 0;
  const update = (dt: number, followX = 0, followZ = 0): void => {
    t += dt;
    if (spindrift) {
      spindriftEmitter.set(followX, 0.2, followZ);
      spindrift.emitter = spindriftEmitter;
      spindrift.emitRate = 90 + Math.sin(t * 0.28) * 30;
    }
  };

  return {
    sun,
    shadow: null,
    sunDir,
    sh0,
    sh1,
    sh2,
    skyTint,
    skyLut,
    update,
    dispose: () => {
      spindrift?.dispose();
      skyLut?.dispose();
    },
  };
}

function buildSkyDome(scene: Scene, sunDir: Vector3, fogColor: Color3): void {
  const dome = MeshBuilder.CreateSphere("skyDome", { diameter: 1200, segments: 28 }, scene);
  dome.infiniteDistance = true;
  dome.isPickable = false;
  dome.receiveShadows = false;

  const fallback = new StandardMaterial("skyMatFallback", scene);
  fallback.emissiveColor = new Color3(0.4, 0.52, 0.72);
  fallback.disableLighting = true;
  fallback.backFaceCulling = false;
  fallback.disableDepthWrite = true;
  dome.material = fallback;

  ShaderStore.ShadersStoreWGSL["skyDomeVertexShader"] = SKY_VERT;
  ShaderStore.ShadersStoreWGSL["skyDomeFragmentShader"] = SKY_FRAG;
  const mat = new ShaderMaterial(
    "skyMat",
    scene,
    { vertex: "skyDome", fragment: "skyDome" },
    {
      attributes: ["position"],
      uniforms: ["viewProjection", "sunDir", "sunColor", "groundBounce", "fogColor"],
      shaderLanguage: ShaderLanguage.WGSL,
    },
  );
  mat.backFaceCulling = false;
  mat.disableDepthWrite = true;
  mat.setVector3("sunDir", sunDir);
  mat.setColor3("sunColor", new Color3(1.0, 0.88, 0.7));
  mat.setColor3("groundBounce", new Color3(0.83, 0.86, 0.91));
  mat.setColor3("fogColor", fogColor);

  void whenReady(mat, "skyDome", [dome], 45000)
    .then(() => {
      dome.material = mat;
    })
    .catch((err) => {
      console.warn("[atmosphere] sky WGSL unavailable, keeping emissive fallback:", err);
    });
}

/** Small equirect LUT for water refraction sampling. */
function bakeSkyLut(scene: Scene, sunDir: Vector3): RawTexture {
  const w = 256;
  const h = 128;
  const data = new Uint8Array(w * h * 4);
  for (let y = 0; y < h; y++) {
    const v = y / (h - 1);
    const elev = (0.5 - v) * Math.PI;
    for (let x = 0; x < w; x++) {
      const u = x / (w - 1);
      const az = (u * 2 - 1) * Math.PI;
      const dir = new Vector3(
        Math.cos(elev) * Math.sin(az),
        Math.sin(elev),
        Math.cos(elev) * Math.cos(az),
      );
      dir.normalize();
      const mu = Math.max(0, Vector3.Dot(dir, sunDir));
      const up = Math.max(dir.y, 0);
      const r = Math.min(255, (80 + up * 90 + mu * mu * 40) | 0);
      const g = Math.min(255, (110 + up * 100 + mu * 30) | 0);
      const b = Math.min(255, (160 + up * 70 + (1 - mu) * 20) | 0);
      const i = (y * w + x) * 4;
      data[i] = r;
      data[i + 1] = g;
      data[i + 2] = b;
      data[i + 3] = 255;
    }
  }
  const tex = RawTexture.CreateRGBATexture(
    data,
    w,
    h,
    scene,
    false,
    false,
    Texture.BILINEAR_SAMPLINGMODE,
  );
  tex.wrapU = Texture.WRAP_ADDRESSMODE;
  tex.wrapV = Texture.CLAMP_ADDRESSMODE;
  return tex;
}

function buildSpindrift(scene: Scene): ParticleSystem {
  const ps = new ParticleSystem("spindrift", 400, scene);
  ps.particleTexture = new Texture(
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==",
    scene,
  );
  ps.minSize = 0.08;
  ps.maxSize = 0.35;
  ps.minLifeTime = 0.6;
  ps.maxLifeTime = 1.8;
  ps.emitRate = 100;
  ps.color1 = new Color4(0.95, 0.97, 1, 0.35);
  ps.color2 = new Color4(0.85, 0.9, 0.98, 0.1);
  ps.gravity = new Vector3(0, -0.2, 0);
  ps.direction1 = new Vector3(-1, 0.05, -0.2);
  ps.direction2 = new Vector3(1, 0.2, 0.5);
  ps.minEmitPower = 0.4;
  ps.maxEmitPower = 1.2;
  ps.updateSpeed = 0.016;
  ps.blendMode = ParticleSystem.BLENDMODE_STANDARD;
  ps.start();
  return ps;
}
