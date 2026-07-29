import { Color3, MeshBuilder, Scene, StandardMaterial } from "@babylonjs/core";
import type { EffectNode } from "../../manifest.ts";
import type { EffectContext, EffectInstance } from "./ribbon.ts";

export function createCrystalEffect(scene: Scene, node: EffectNode): EffectInstance {
  const scale = (node.props?.scaleM as number) ?? (node.props?.scale_m as number) ?? 1;
  const crystals: ReturnType<typeof MeshBuilder.CreatePolyhedron>[] = [];
  let growth = 0;

  for (let i = 0; i < 7; i++) {
    const c = MeshBuilder.CreatePolyhedron(`crystal${i}`, { type: 1, size: 0.01 }, scene);
    c.isVisible = false;
    const mat = new StandardMaterial(`crystalMat${i}`, scene);
    mat.diffuseColor = new Color3(0.55 + (i % 3) * 0.08, 0.82, 0.98);
    mat.alpha = 0.72;
    mat.specularColor = new Color3(1, 1, 1);
    mat.emissiveColor = new Color3(0.12, 0.22, 0.32);
    mat.backFaceCulling = false;
    c.material = mat;
    crystals.push(c);
  }

  return {
    update(ctx: EffectContext) {
      growth = Math.min(1, growth + ctx.dt * 0.55);
      const ease = growth * growth * (3 - 2 * growth);
      const tx = ctx.origin.x + ctx.aim.x * 2;
      const tz = ctx.origin.z + ctx.aim.z * 2;
      const gy = ctx.character.groundY(tx, tz);
      for (let i = 0; i < crystals.length; i++) {
        const c = crystals[i]!;
        c.isVisible = growth > 0.05;
        const ang = i * 0.95 + growth;
        const rad = 0.15 + (i % 3) * 0.18 * ease;
        c.position.x = tx + Math.sin(ang) * rad;
        c.position.y = gy + ease * scale * (0.35 + i * 0.12);
        c.position.z = tz + Math.cos(ang) * rad;
        c.scaling.setAll(ease * scale * (0.22 + (i % 3) * 0.08));
        c.rotation.y = ang;
        c.rotation.x = 0.2 + i * 0.05;
      }
      ctx.deformation.writeState(tx, tz, scale, "ice", ease);
      ctx.deformation.brush(tx, tz, scale * 0.8, 0.04 * ease, {
        ice: ease,
        compression: 0.3,
        shape: "circle",
      });
    },
    warmup() {
      crystals[0]!.isVisible = true;
      crystals[0]!.scaling.setAll(0.01);
    },
    dispose() {
      for (const c of crystals) c.dispose();
    },
  };
}
