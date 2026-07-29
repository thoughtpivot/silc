import type { EffectNode } from "../../manifest.ts";
import type { DeformationField } from "../../terrain/deformation.ts";
import type { CharacterController } from "../../character/controller.ts";
import type { SpringArmCamera } from "../../camera/springArm.ts";
import type { WaterBody } from "../waterBody.ts";
import { STRAND_COLS } from "../waterBody.ts";
import { RibbonBuffer, scratchV3a } from "../../core/pools.ts";

export type EffectContext = {
  deformation: DeformationField;
  character: CharacterController;
  camera: SpringArmCamera;
  time: number;
  dt: number;
  origin: { x: number; y: number; z: number };
  aim: { x: number; y: number; z: number };
  water?: WaterBody | null;
};

export type EffectInstance = {
  update: (ctx: EffectContext) => void;
  warmup: () => void;
  dispose: () => void;
};

export function createRibbonEffect(node: EffectNode): EffectInstance {
  const width =
    (node.props?.widthM as number | undefined) ??
    (node.props?.width_m as number | undefined) ??
    0.4;
  const buffer = new RibbonBuffer(96);
  let t = 0;
  let life = 0;
  let strand = -1;
  let waterRef: WaterBody | null = null;

  return {
    update(ctx) {
      t += ctx.dt;
      life += ctx.dt;
      const ease = Math.min(1, life * 2.2);
      const smooth = ease * ease * (3 - 2 * ease);
      const reach = 1.5 + smooth * 2.5;
      scratchV3a.x = ctx.origin.x + ctx.aim.x * reach;
      scratchV3a.y = ctx.origin.y + 0.55 + Math.sin(t * 4) * 0.18;
      scratchV3a.z = ctx.origin.z + ctx.aim.z * reach;
      buffer.push(scratchV3a, width * (0.6 + smooth * 0.5), t);

      if (ctx.water) {
        waterRef = ctx.water;
        if (strand < 0) strand = ctx.water.acquire();
        if (strand >= 0) {
          ctx.water.mesh.isVisible = true;
          const n = Math.min(buffer.count, STRAND_COLS);
          const rightX = -ctx.aim.z;
          const rightZ = ctx.aim.x;
          for (let i = 0; i < STRAND_COLS; i++) {
            const src =
              n <= 1
                ? 0
                : Math.min(n - 1, Math.floor((i / (STRAND_COLS - 1)) * (n - 1)));
            const pt = buffer.points[(buffer.head - n + src + buffer.maxPoints) % buffer.maxPoints]!;
            const radius = (pt?.width ?? width) * (0.35 + smooth * 0.4);
            ctx.water.column(
              strand,
              i,
              pt?.position.x ?? scratchV3a.x,
              pt?.position.y ?? scratchV3a.y,
              pt?.position.z ?? scratchV3a.z,
              radius,
              rightX,
              0,
              rightZ,
              0.4 + Math.sin(t * 6 + i * 0.2) * 0.2,
            );
          }
        }
      }

      ctx.deformation.brush(scratchV3a.x, scratchV3a.z, width * 0.55, 0.06 * smooth, {
        wetness: 0.85,
        shape: "score",
      });
      if (buffer.count > 2) {
        const prev = buffer.points[(buffer.head + buffer.count - 2) % buffer.maxPoints]!;
        ctx.deformation.brush(prev.position.x, prev.position.z, width * 0.35, 0.03 * smooth, {
          wetness: 0.5,
          shape: "score",
        });
      }
    },
    warmup() {
      buffer.push(scratchV3a, width, 0);
    },
    dispose() {
      buffer.clear();
      if (strand >= 0 && waterRef) waterRef.release(strand);
      strand = -1;
      waterRef = null;
    },
  };
}
