import { Color3, Color4, ParticleSystem, Scene, Texture, Vector3 } from "@babylonjs/core";
import type { EffectNode } from "../../manifest.ts";
import type { EffectContext, EffectInstance } from "./ribbon.ts";
import { scratchV3a, scratchV3b } from "../../core/pools.ts";

const TEX =
  "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg==";

export function createParticleEffect(scene: Scene, node: EffectNode): EffectInstance {
  const kind = (node.props?.kind as string) ?? "spray";
  const count = (node.props?.count as number) ?? 100;
  const ps = new ParticleSystem(`particles_${kind}`, count, scene);
  ps.particleTexture = new Texture(TEX, scene);
  ps.emitter = scratchV3a;
  ps.minEmitBox = new Vector3(-0.4, 0, -0.4);
  ps.maxEmitBox = new Vector3(0.4, 0.8, 0.4);
  ps.color1 = new Color4(0.92, 0.96, 1, 0.85);
  ps.color2 = new Color4(0.72, 0.86, 1, 0.15);
  ps.minSize = kind === "sparkle" ? 0.02 : 0.03;
  ps.maxSize = kind === "powder" || kind === "drift" ? 0.28 : 0.12;
  ps.minLifeTime = 0.35;
  ps.maxLifeTime = kind === "drift" || kind === "vortex" ? 2.8 : 1.2;
  ps.emitRate = 0;
  ps.gravity = new Vector3(0, kind === "powder" || kind === "drift" ? -1.2 : -4.2, 0);
  ps.updateSpeed = 0.02;
  ps.blendMode = ParticleSystem.BLENDMODE_STANDARD;
  ps.start();

  let life = 0;
  const dir1 = new Vector3();
  const dir2 = new Vector3();

  return {
    update(ctx: EffectContext) {
      life += ctx.dt;
      scratchV3a.set(ctx.origin.x, ctx.origin.y + 0.45, ctx.origin.z);
      ps.emitter = scratchV3a;
      const ease = Math.min(1, life * 3);
      if (kind === "drift" || kind === "vortex") {
        dir1.set(-2.5, 2.5, -2.5);
        dir2.set(2.5, 7, 2.5);
        ps.direction1 = dir1;
        ps.direction2 = dir2;
        ps.emitRate = 380 * ease;
        ctx.deformation.brush(ctx.origin.x, ctx.origin.z, 2.2 * ease, 0.08 * ctx.dt * 6, {
          shape: "ring",
        });
      } else if (kind === "powder") {
        dir1.set(-1.2, 3.5, -1.2);
        dir2.set(1.2, 8, 1.2);
        ps.direction1 = dir1;
        ps.direction2 = dir2;
        ps.emitRate = 320 * ease;
      } else if (kind === "sparkle") {
        dir1.set(-0.4, 0.5, -0.4);
        dir2.set(0.4, 2, 0.4);
        ps.direction1 = dir1;
        ps.direction2 = dir2;
        ps.emitRate = 90 * ease;
      } else {
        dir1.set(ctx.aim.x * 2, 0.5, ctx.aim.z * 2);
        dir2.set(ctx.aim.x * 4, 2.5, ctx.aim.z * 4);
        ps.direction1 = dir1;
        ps.direction2 = dir2;
        ps.emitRate = 140 * ease;
      }
      void Color3;
      void scratchV3b;
    },
    warmup() {
      life = 0.5;
      ps.emitRate = 20;
    },
    dispose() {
      ps.stop();
      ps.dispose();
    },
  };
}
