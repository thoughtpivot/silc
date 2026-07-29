import type { EffectNode } from "../../manifest.ts";
import type { EffectContext, EffectInstance } from "./ribbon.ts";
import { STRAND_COLS } from "../waterBody.ts";
import type { WaterBody } from "../waterBody.ts";

export function createBrushEffect(node: EffectNode): EffectInstance {
  const depth =
    (node.props?.depthM as number | undefined) ??
    (node.props?.depth_m as number | undefined) ??
    0.2;
  const radius =
    (node.props?.radiusM as number | undefined) ??
    (node.props?.radius_m as number | undefined) ??
    1.5;
  const shape = (node.props?.shape as string) ?? "circle";
  let life = 0;
  let strand = -1;
  let waterRef: WaterBody | null = null;

  const wantsWater =
    shape === "crescent" || shape === "channel" || shape === "groove" || shape === "wave";

  return {
    update(ctx: EffectContext) {
      life += ctx.dt;
      const ease = Math.min(1, life * 2.5);
      const smooth = ease * ease * (3 - 2 * ease);
      const tx = ctx.origin.x + ctx.aim.x * 3;
      const tz = ctx.origin.z + ctx.aim.z * 3;
      const ty = ctx.origin.y + 0.2 + smooth * 0.8;

      ctx.deformation.brush(tx, tz, radius * (0.7 + smooth * 0.3), depth * ctx.dt * 5 * smooth, {
        shape,
        wetness: wantsWater ? 0.7 : 0,
        yaw: Math.atan2(ctx.aim.x, ctx.aim.z),
      });

      if (wantsWater && ctx.water) {
        waterRef = ctx.water;
        if (strand < 0) strand = ctx.water.acquire();
        if (strand >= 0) {
          ctx.water.mesh.isVisible = true;
          const rightX = -ctx.aim.z;
          const rightZ = ctx.aim.x;
          for (let c = 0; c < STRAND_COLS; c++) {
            const t = c / (STRAND_COLS - 1);
            const along = 0.4 + t * (2.5 + radius);
            const arc = Math.sin(t * Math.PI) * radius * 0.55 * smooth;
            ctx.water.column(
              strand,
              c,
              ctx.origin.x + ctx.aim.x * along + rightX * arc,
              ty + Math.sin(t * Math.PI) * 0.6 * smooth,
              ctx.origin.z + ctx.aim.z * along + rightZ * arc,
              (0.12 + (1 - Math.abs(t - 0.5) * 2) * 0.25) * smooth * radius * 0.35,
              rightX,
              0.15,
              rightZ,
              0.55 * smooth,
            );
          }
        }
      }
    },
    warmup() {
      life = 0.5;
    },
    dispose() {
      if (strand >= 0 && waterRef) waterRef.release(strand);
      strand = -1;
      waterRef = null;
    },
  };
}
