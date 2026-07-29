import { Color3, PointLight, Scene, Vector3 } from "@babylonjs/core";
import type { EffectNode } from "../../manifest.ts";
import type { EffectContext, EffectInstance } from "./ribbon.ts";

function parseColor(hex: string): Color3 {
  const h = hex.replace("#", "");
  const r = parseInt(h.substring(0, 2), 16) / 255;
  const g = parseInt(h.substring(2, 4), 16) / 255;
  const b = parseInt(h.substring(4, 6), 16) / 255;
  return new Color3(r, g, b);
}

export type SpellLightState = {
  pos: Vector3;
  color: Color3;
  intensity: number;
  range: number;
};

/** Shared spell light sampled by the snow material for SSS coupling. */
export const activeSpellLight: SpellLightState = {
  pos: new Vector3(),
  color: new Color3(1, 0.9, 0.75),
  intensity: 0,
  range: 6,
};

export function createLightEffect(scene: Scene, node: EffectNode): EffectInstance {
  const radius =
    (node.props?.radiusM as number | undefined) ??
    (node.props?.radius_m as number | undefined) ??
    6;
  const intensity = (node.props?.intensity as number) ?? 2;
  const color = parseColor((node.props?.color as string) ?? "#ffe8c0");
  const light = new PointLight("spellLight", Vector3.Zero(), scene);
  light.diffuse = color;
  light.intensity = 0;
  light.range = radius;
  let life = 0;

  return {
    update(ctx: EffectContext) {
      life += ctx.dt;
      const easeIn = Math.min(1, life * 4);
      const easeOut = Math.max(0, 1 - Math.max(0, life - 2.2) * 0.55);
      const amp = intensity * easeIn * easeOut;
      light.position.x = ctx.origin.x + ctx.aim.x * 2.2;
      light.position.y = ctx.origin.y + 1.1;
      light.position.z = ctx.origin.z + ctx.aim.z * 2.2;
      light.intensity = amp;
      activeSpellLight.pos.copyFrom(light.position);
      activeSpellLight.color.copyFrom(color);
      activeSpellLight.intensity = amp;
      activeSpellLight.range = radius;
    },
    warmup() {
      light.intensity = intensity * 0.5;
      activeSpellLight.intensity = intensity * 0.5;
      activeSpellLight.range = radius;
    },
    dispose() {
      light.dispose();
      activeSpellLight.intensity = 0;
    },
  };
}
