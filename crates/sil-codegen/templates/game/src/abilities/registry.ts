import type { Scene } from "@babylonjs/core";
import type { AbilityDef, EffectNode, GameManifest } from "../manifest.ts";
import type { DeformationField } from "../terrain/deformation.ts";
import type { CharacterController } from "../character/controller.ts";
import type { SpringArmCamera } from "../camera/springArm.ts";
import type { InputController } from "../controls/input.ts";
import { buildSpellFromManifest } from "./spells.ts";
import type { EffectContext, EffectInstance } from "./effects/ribbon.ts";
import { createRibbonEffect } from "./effects/ribbon.ts";
import { createParticleEffect } from "./effects/particles.ts";
import { createBrushEffect } from "./effects/brush.ts";
import { createStateWriteEffect } from "./effects/stateWrite.ts";
import { createCrystalEffect } from "./effects/crystal.ts";
import { createLightEffect } from "./effects/light.ts";
import { createImpulseEffect } from "./effects/impulse.ts";
import type { WaterBody } from "./waterBody.ts";

type ActiveAbility = {
  key: string;
  name: string;
  effects: EffectInstance[];
  duration: number;
  elapsed: number;
  waterStrand: number;
};

export class AbilityRegistry {
  private readonly defs: AbilityDef[];
  private readonly scene: Scene;
  private readonly deformation: DeformationField;
  private readonly character: CharacterController;
  private readonly camera: SpringArmCamera;
  private readonly water: WaterBody | null;
  private active: ActiveAbility | null = null;
  private time = 0;
  private readonly keyLatch = new Set<string>();

  constructor(
    scene: Scene,
    manifest: GameManifest,
    deformation: DeformationField,
    character: CharacterController,
    camera: SpringArmCamera,
    water: WaterBody | null = null,
  ) {
    this.scene = scene;
    this.defs = manifest.abilities ?? [];
    this.deformation = deformation;
    this.character = character;
    this.camera = camera;
    this.water = water;
  }

  private instantiateEffect(node: EffectNode): EffectInstance {
    switch (node.type) {
      case "ribbon":
        return createRibbonEffect(node);
      case "particle_emitter":
        return createParticleEffect(this.scene, node);
      case "terrain_brush":
        return createBrushEffect(node);
      case "state_write":
        return createStateWriteEffect(node);
      case "crystal_growth":
        return createCrystalEffect(this.scene, node);
      case "dynamic_light":
        return createLightEffect(this.scene, node);
      case "camera_impulse":
        return createImpulseEffect(node);
      case "wake": {
        const intensity =
          typeof node.props?.intensity === "number" ? node.props.intensity : 1.0;
        return createBrushEffect({
          type: "terrain_brush",
          props: {
            shape: "crescent",
            depthM: 0.2 * intensity,
            radiusM: 1.5 * Math.max(0.5, intensity),
          },
        });
      }
      default:
        return createBrushEffect(node);
    }
  }

  private buildEffects(nodes: EffectNode[]): EffectInstance[] {
    const out: EffectInstance[] = [];
    for (let i = 0; i < nodes.length; i++) {
      out.push(this.instantiateEffect(nodes[i]!));
    }
    return out;
  }

  cast(key: string): void {
    for (let i = 0; i < this.defs.length; i++) {
      const def = this.defs[i]!;
      if (def.key !== key) continue;
      if (this.active) {
        for (const e of this.active.effects) e.dispose();
        if (this.active.waterStrand >= 0) this.water?.release(this.active.waterStrand);
      }
      this.active = {
        key,
        name: def.name,
        effects: buildSpellFromManifest(def, (n) => this.instantiateEffect(n)),
        duration: def.name === "Ribbon" ? 999 : 4,
        elapsed: 0,
        waterStrand: -1,
      };
      return;
    }
  }

  update(dt: number, input: InputController): void {
    this.time += dt;
    const key = input.abilityKeyPressed();
    if (key && !this.keyLatch.has(key)) {
      this.cast(key);
      this.keyLatch.add(key);
    }
    if (!key) this.keyLatch.clear();

    if (this.active) {
      this.active.elapsed += dt;
      const ctx = this.makeContext(dt);
      for (let i = 0; i < this.active.effects.length; i++) {
        this.active.effects[i]!.update(ctx);
      }
      if (this.active.elapsed >= this.active.duration && this.active.name !== "Ribbon") {
        for (const e of this.active.effects) e.dispose();
        if (this.active.waterStrand >= 0) this.water?.release(this.active.waterStrand);
        this.active = null;
      }
    }

    if (key === "2" && this.active?.name === "Ribbon") {
      // held ribbon
    } else if (this.active?.name === "Ribbon" && !key) {
      for (const e of this.active.effects) e.dispose();
      if (this.active.waterStrand >= 0) this.water?.release(this.active.waterStrand);
      this.active = null;
    }
  }

  private makeContext(dt: number): EffectContext {
    const pos = this.character.position;
    const yaw = this.camera.yaw;
    return {
      deformation: this.deformation,
      character: this.character,
      camera: this.camera,
      time: this.time,
      dt,
      origin: { x: pos.x, y: pos.y + 1.1, z: pos.z },
      aim: { x: Math.sin(yaw), y: 0, z: Math.cos(yaw) },
      water: this.water,
    };
  }

  warmup(): void {
    for (let i = 0; i < this.defs.length; i++) {
      const effects = this.buildEffects(this.defs[i]!.effects);
      for (let j = 0; j < effects.length; j++) {
        effects[j]!.warmup();
        effects[j]!.update(this.makeContext(1 / 60));
      }
      for (const e of effects) e.dispose();
    }
  }

  dispose(): void {
    if (this.active) {
      for (const e of this.active.effects) e.dispose();
      if (this.active.waterStrand >= 0) this.water?.release(this.active.waterStrand);
    }
  }
}
