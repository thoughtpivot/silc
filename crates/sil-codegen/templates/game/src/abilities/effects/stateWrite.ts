import type { EffectNode } from "../../manifest.ts";
import type { EffectContext, EffectInstance } from "./ribbon.ts";

export function createStateWriteEffect(node: EffectNode): EffectInstance {
  const channel = (node.props?.channel as string) ?? "compression";
  const amount = (node.props?.amount as number) ?? 0.5;
  const radius = (node.props?.radiusM as number) ?? 1.2;

  return {
    update(ctx: EffectContext) {
      const tx = ctx.origin.x + ctx.aim.x * 2;
      const tz = ctx.origin.z + ctx.aim.z * 2;
      ctx.deformation.writeState(tx, tz, radius, channel, amount * ctx.dt * 2);
    },
    warmup() {},
    dispose() {},
  };
}
