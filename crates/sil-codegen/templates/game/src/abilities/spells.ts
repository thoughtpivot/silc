import type { AbilityDef, EffectNode } from "../manifest.ts";
import type { EffectInstance } from "./effects/ribbon.ts";

/** Compose manifest ability effect trees into runnable instances. */
export function buildSpellFromManifest(
  def: AbilityDef,
  instantiate: (node: EffectNode) => EffectInstance,
): EffectInstance[] {
  const effects: EffectInstance[] = [];
  for (let i = 0; i < def.effects.length; i++) {
    effects.push(instantiate(def.effects[i]!));
  }
  return effects;
}

export const SPELL_NAMES = ["Sweep", "Ribbon", "Bloom", "Crystallize", "Vortex"] as const;

export type SpellName = (typeof SPELL_NAMES)[number];

export function spellDuration(name: SpellName): number {
  switch (name) {
    case "Ribbon":
      return 999;
    case "Vortex":
      return 5;
    case "Crystallize":
      return 6;
    default:
      return 4;
  }
}
