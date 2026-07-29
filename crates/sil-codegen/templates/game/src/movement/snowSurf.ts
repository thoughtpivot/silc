import type { EffectNode, GameManifest } from "../manifest.ts";
import type { DeformationField } from "../terrain/deformation.ts";
import type { CharacterController } from "../character/controller.ts";
import type { SpringArmCamera } from "../camera/springArm.ts";
import type { Scene } from "@babylonjs/core";
import { SurfWake } from "./surfWake.ts";
import { scratchV3a } from "../core/pools.ts";

type SurfBrush = {
  shape: string;
  depthM: number;
  radiusM: number;
};

export class SnowSurfController {
  private active = false;
  private speed = 0;
  private readonly maxSpeed: number;
  private readonly accel: number;
  private turnRate = 0;
  private easeT = 0;
  private readonly wakeIntensity: number;
  private readonly groove: SurfBrush;
  private readonly compression: number;
  readonly wake: SurfWake;

  constructor(
    scene: Scene,
    manifest: GameManifest,
    private readonly deformation: DeformationField,
    private readonly character: CharacterController,
    private readonly camera: SpringArmCamera,
    sunDir: { x: number; y: number; z: number },
  ) {
    this.maxSpeed = 14;
    this.accel = 18;
    const mode =
      (manifest.movementModes ?? []).find((m) => m.name === "snow_surf" || m.hold === "RMB") ??
      (manifest.movementModes ?? [])[0];
    const effects = mode?.effects ?? [];
    this.wakeIntensity = numProp(effects, "wake", "intensity", 1.2);
    this.groove = {
      shape: strProp(effects, "terrain_brush", "shape", "groove"),
      depthM: numProp(effects, "terrain_brush", "depth_m", 0.35),
      radiusM: numProp(effects, "terrain_brush", "radius_m", 1.2),
    };
    this.compression = numProp(effects, "state_write", "amount", 0.8);
    this.wake = new SurfWake(scene, sunDir as never);
  }

  setActive(on: boolean): void {
    if (on && !this.active) this.easeT = 0;
    if (!on && this.active) {
      this.easeT = 0;
      this.speed *= 0.4;
    }
    this.active = on;
  }

  getSpeed(): number {
    return this.speed * (this.active ? 1 : 0);
  }

  update(dt: number, mouseDx: number, yaw: number, scene: Scene): void {
    this.easeT = Math.min(1, this.easeT + dt * 1.8);
    const ease = this.easeT * this.easeT * (3 - 2 * this.easeT);

    if (this.active) {
      this.speed = Math.min(this.maxSpeed, this.speed + this.accel * dt * ease);
      this.turnRate += (-mouseDx * 0.0035 - this.turnRate) * 0.18;
      const steerYaw = yaw + this.turnRate * (this.speed / this.maxSpeed);

      scratchV3a.x = Math.sin(steerYaw);
      scratchV3a.z = Math.cos(steerYaw);
      this.character.velocity.x = scratchV3a.x * this.speed;
      this.character.velocity.z = scratchV3a.z * this.speed;
      this.character.position.x += this.character.velocity.x * dt;
      this.character.position.z += this.character.velocity.z * dt;
      this.character.root.rotation.y = steerYaw;
      this.character.root.rotation.z = this.turnRate * 0.45;
    }

    const px = this.character.position.x;
    const pz = this.character.position.z;
    const py = this.character.position.y;
    this.wake.update(
      dt,
      this.active,
      px,
      py,
      pz,
      this.character.root.rotation.y,
      this.speed * this.wakeIntensity,
      this.turnRate,
      this.deformation,
      scene,
    );

    void this.camera;
    void this.groove;
    void this.compression;
  }

  dispose(): void {
    this.speed = 0;
    this.wake.dispose();
  }
}

function findEffect(effects: EffectNode[], type: string): EffectNode | undefined {
  for (let i = 0; i < effects.length; i++) {
    if (effects[i]!.type === type) return effects[i];
  }
  return undefined;
}

function numProp(effects: EffectNode[], type: string, key: string, fallback: number): number {
  const node = findEffect(effects, type);
  const camel = key.replace(/_([a-z])/g, (_: string, c: string) => c.toUpperCase());
  const value = node?.props?.[key] ?? node?.props?.[camel];
  return typeof value === "number" ? value : fallback;
}

function strProp(effects: EffectNode[], type: string, key: string, fallback: string): string {
  const node = findEffect(effects, type);
  const value = node?.props?.[key];
  return typeof value === "string" ? value : fallback;
}
