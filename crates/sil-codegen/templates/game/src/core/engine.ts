import { ArcRotateCamera, Scene, Vector3, WebGPUEngine } from "@babylonjs/core";
import type { GameManifest } from "../manifest.ts";
import { DeformationField } from "../terrain/deformation.ts";
import { Heightfield } from "../terrain/heightfield.ts";
import { TerrainClipmap } from "../terrain/clipmap.ts";
import { setupAtmosphere, type AtmosphereHandle } from "../environment/atmosphere.ts";
import { createPostChain, type PostChainHandle } from "../post/chain.ts";
import { InputController } from "../controls/input.ts";
import { SpringArmCamera } from "../camera/springArm.ts";
import { CharacterController } from "../character/controller.ts";
import { RobeFigure } from "../character/robe.ts";
import { SnowSurfController } from "../movement/snowSurf.ts";
import { AbilityRegistry } from "../abilities/registry.ts";
import { WaterBody } from "../abilities/waterBody.ts";
import { SettingsOverlay } from "../ui/overlay.ts";
import { activeSpellLight } from "../abilities/effects/light.ts";

export type GameEngine = {
  engine: WebGPUEngine;
  scene: Scene;
  manifest: GameManifest;
  deformation: DeformationField;
  heightfield: Heightfield;
  terrain: TerrainClipmap;
  atmosphere: AtmosphereHandle;
  post: PostChainHandle;
  input: InputController;
  camera: SpringArmCamera;
  character: CharacterController;
  robe: RobeFigure;
  surf: SnowSurfController;
  water: WaterBody;
  abilities: AbilityRegistry;
  overlay: SettingsOverlay;
  update: (dt: number) => void;
  render: () => void;
  dispose: () => void;
};

export async function createGameEngine(
  canvas: HTMLCanvasElement,
  manifest: GameManifest,
  onProgress?: (label: string, pct: number) => void,
): Promise<GameEngine> {
  const report = onProgress ?? (() => undefined);

  report("Starting WebGPU device", 0.1);
  const engine = await initWebGpuEngine(canvas, report);
  // Resize after device exists — setting canvas.width before initAsync destroys
  // the swapchain texture mid-frame on WebGPU.
  engine.resize();
  if (!engine.getCaps().textureFloatLinearFiltering) {
    console.warn("[silc-game] float32-filterable unavailable; height will step/fail");
  }

  const scene = new Scene(engine);
  scene.clearColor = { r: 0.45, g: 0.58, b: 0.78, a: 1 } as never;
  scene.autoClear = true;
  // Must stay false until materials have bound — otherwise ShaderMaterials never
  // pick up textures and the canvas stays blank after the loading screen.
  scene.blockMaterialDirtyMechanism = false;

  report("Deformation buffer", 0.12);
  const deformation = new DeformationField(scene, manifest);

  report("Heightfield bake", 0.18);
  const heightfield = new Heightfield(scene, manifest);
  await heightfield.bake(report);

  report("Clipmap terrain", 0.55);
  const terrain = new TerrainClipmap(scene, manifest, deformation, heightfield);

  report("Atmosphere & sky", 0.62);
  const atmosphere = setupAtmosphere(scene, manifest);
  terrain.material?.setSkySH(atmosphere.sh0, atmosphere.sh1, atmosphere.sh2, atmosphere.skyTint);

  const input = new InputController(manifest);
  const camTarget = new Vector3(0, Math.max(1.4, heightfield.heightAt(0, 0) + 1.4), 0);
  // Prefer a longer, more horizontal arm so dunes + sky read on first paint.
  const arcCam = new ArcRotateCamera("arc", -Math.PI * 0.45, 1.18, 12, camTarget, scene);
  arcCam.lowerRadiusLimit = 2.5;
  arcCam.upperRadiusLimit = 36;
  arcCam.lowerBetaLimit = 0.4;
  arcCam.upperBetaLimit = 1.42;
  arcCam.minZ = 0.15;
  arcCam.maxZ = 2000;
  arcCam.inputs.clear();
  scene.activeCamera = arcCam;
  // Single resize after camera exists — avoid thrashing the WebGPU swapchain.
  const post = createPostChain(scene, arcCam, manifest, { bypass: false });
  const camera = new SpringArmCamera(arcCam, manifest);
  const character = new CharacterController(scene, manifest, heightfield);
  character.position.set(0, heightfield.heightAt(0, 0), 0);
  character.root.position.copyFrom(character.position);
  const robe = new RobeFigure(scene, manifest, character.root);

  const water = new WaterBody(scene, atmosphere.skyLut, atmosphere.sunDir);
  water.mesh.isVisible = false;
  const surf = new SnowSurfController(
    scene,
    manifest,
    deformation,
    character,
    camera,
    atmosphere.sunDir,
  );

  const abilities = new AbilityRegistry(
    scene,
    manifest,
    deformation,
    character,
    camera,
    water,
  );
  const overlay = new SettingsOverlay(manifest, post, deformation, atmosphere);

  // Promote deform to GPU after first materials exist (CPU path already live).
  void deformation.warmGpu().catch((err) => console.warn("[deformation] warmGpu:", err));

  character.onFootfall = (x, z) => {
    deformation.brush(x, z, 0.22, 0.08, { compression: 0.35, shape: "circle" });
    robe.pulseFootSpray();
  };

  input.attach(canvas);
  overlay.attach(input);

  // First camera settle so the first render isn't looking at the wrong height.
  camera.update(1 / 60, character.position, { dx: 0, dy: 0, wheel: 0 }, 0);
  terrain.update(character.position.x, character.position.z, deformation);

  let lastRefill = 0;

  const update = (dt: number): void => {
    input.update();
    const mouse = input.consumeMouse();

    const surfActive = input.holdRmb;
    surf.setActive(surfActive);
    camera.update(dt, character.position, mouse, surf.getSpeed());
    character.update(dt, input, camera, surfActive);
    surf.update(dt, mouse.dx, camera.yaw, scene);

    deformation.follow(character.position.x, character.position.z);
    terrain.update(character.position.x, character.position.z, deformation);

    lastRefill += dt;
    if (lastRefill > 0.05) {
      deformation.refill(lastRefill);
      lastRefill = 0;
    }

    abilities.update(dt, input);
    const anyWater = waterHasActiveStrand(water);
    water.mesh.isVisible = anyWater;
    water.update(dt, scene);
    robe.update(dt, character.velocity, surfActive);
    atmosphere.update(dt, character.position.x, character.position.z);

    const sink = deformation.sampleDepression(character.position.x, character.position.z) * 0.35;
    character.position.y = heightfield.heightAt(character.position.x, character.position.z) - sink;
    character.root.position.copyFrom(character.position);

    const snow = terrain.material;
    if (snow) {
      snow.setSpellLight(
        activeSpellLight.pos,
        activeSpellLight.color,
        activeSpellLight.intensity,
        activeSpellLight.range,
      );
    }

    deformation.upload();
    overlay.tick(input, engine, scene.getTotalVertices(), scene.getActiveMeshes().length);
  };

  const render = (): void => {
    scene.render();
  };

  const dispose = (): void => {
    overlay.dispose();
    abilities.dispose();
    water.dispose();
    surf.dispose();
    robe.dispose();
    character.dispose();
    post.dispose();
    atmosphere.dispose();
    terrain.dispose();
    heightfield.dispose();
    deformation.dispose();
    scene.dispose();
    engine.dispose();
  };

  report("Engine ready", 0.72);

  return {
    engine,
    scene,
    manifest,
    deformation,
    heightfield,
    terrain,
    atmosphere,
    post,
    input,
    camera,
    character,
    robe,
    surf,
    water,
    abilities,
    overlay,
    update,
    render,
    dispose,
  };
}

function waterHasActiveStrand(water: WaterBody): boolean {
  return water.hasActiveStrand();
}

export function startRenderLoop(game: GameEngine, targetFps: number): void {
  const targetDt = 1 / targetFps;
  let last = performance.now();
  game.engine.runRenderLoop(() => {
    const now = performance.now();
    const dt = Math.min(0.05, (now - last) / 1000);
    last = now;
    game.update(dt);
    game.render();
  });
  game.engine.setHardwareScalingLevel(1);
  void targetDt;
}

async function initWebGpuEngine(
  canvas: HTMLCanvasElement,
  report: (label: string, pct: number) => void,
): Promise<WebGPUEngine> {
  const attempts: Array<Record<string, unknown>> = [
    {
      antialias: false,
      // Device-ratio adapts fight with explicit resize and can destroy the
      // swapchain mid-submit (IOSurface validation errors on first frames).
      adaptToDeviceRatio: false,
      powerPreference: "high-performance",
      enableAllFeatures: true,
      setMaximumLimits: true,
    },
    {
      antialias: false,
      adaptToDeviceRatio: false,
      powerPreference: "high-performance",
    },
    {
      antialias: false,
      powerPreference: "low-power",
    },
  ];

  let lastErr: unknown = null;
  for (let i = 0; i < attempts.length; i++) {
    const opts = attempts[i]!;
    report(`WebGPU init (${i + 1}/${attempts.length})`, 0.1 + i * 0.02);
    try {
      const engine = new WebGPUEngine(canvas, opts as never);
      await withTimeout(engine.initAsync(), 12_000, `WebGPU init attempt ${i + 1}`);
      engine.resize();
      return engine;
    } catch (err) {
      lastErr = err;
      console.warn("[silc-game] WebGPU init failed:", err);
    }
  }
  throw lastErr instanceof Error
    ? lastErr
    : new Error(`WebGPU init failed: ${String(lastErr)}`);
}

function withTimeout<T>(promise: Promise<T>, ms: number, label: string): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const t = window.setTimeout(() => reject(new Error(`${label} timed out after ${ms}ms`)), ms);
    promise.then(
      (v) => {
        window.clearTimeout(t);
        resolve(v);
      },
      (e) => {
        window.clearTimeout(t);
        reject(e);
      },
    );
  });
}
