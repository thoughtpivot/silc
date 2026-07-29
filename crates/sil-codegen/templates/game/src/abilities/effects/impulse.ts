import type { EffectNode } from "../../manifest.ts";
import type { EffectContext, EffectInstance } from "./ribbon.ts";

export function createImpulseEffect(node: EffectNode): EffectInstance {
  const strength = (node.props?.strength as number) ?? 0.1;
  let fired = false;

  return {
    update(ctx: EffectContext) {
      if (!fired) {
        ctx.camera.applyImpulse(strength);
        fired = true;
      }
    },
    warmup() {},
    dispose() {
      fired = false;
    },
  };
}
