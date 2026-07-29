import { Color3, Mesh, MeshBuilder, Scene, StandardMaterial, TransformNode } from "@babylonjs/core";
import type { GameManifest } from "../manifest.ts";
import type { DeformationField } from "./deformation.ts";
import { Heightfield } from "./heightfield.ts";
import { buildClipmapMesh } from "./clipmapMesh.ts";
import { createSnowMaterial } from "../shaders/snowMaterial.ts";
import { whenReady } from "../core/gpuUtil.ts";

export class TerrainClipmap {
  readonly root: TransformNode;
  /** Single static clipmap mesh (one draw). Kept as `rings[0]` for shadow caster API. */
  readonly rings: Mesh[] = [];
  readonly heightfield: Heightfield;
  /** Fallback ground until snow ShaderMaterial is ready. */
  readonly ground: Mesh;
  private snowMaterial: ReturnType<typeof createSnowMaterial> | null = null;
  private lodX = 0;
  private lodZ = 0;
  /** True once clipmap snow replaced the flat fallback. */
  snowReady = false;

  constructor(
    scene: Scene,
    manifest: GameManifest,
    deformation: DeformationField,
    heightfield: Heightfield,
  ) {
    this.root = new TransformNode("terrainRoot", scene);
    this.heightfield = heightfield;

    const y0 = heightfield.heightAt(0, 0);
    this.ground = MeshBuilder.CreateGround(
      "snowGround",
      { width: 80, height: 80, subdivisions: 64 },
      scene,
    );
    this.ground.parent = this.root;
    this.ground.position.y = y0 - 0.05;
    this.ground.receiveShadows = false;
    this.ground.alwaysSelectAsActiveMesh = true;
    const groundMat = new StandardMaterial("snowGroundMat", scene);
    groundMat.diffuseColor = new Color3(0.88, 0.92, 0.98);
    groundMat.emissiveColor = new Color3(0.2, 0.22, 0.28);
    groundMat.freeze();
    this.ground.material = groundMat;

    const mesh = buildClipmapMesh(scene);
    mesh.parent = this.root;
    this.rings.push(mesh);

    this.snowMaterial = createSnowMaterial(scene, deformation, heightfield, manifest);
    mesh.material = this.snowMaterial.material;
    mesh.isVisible = false;

    void whenReady(this.snowMaterial.material, "snowMaterial", [mesh], 45000)
      .then(() => {
        mesh.isVisible = true;
        this.ground.isVisible = false;
        this.snowReady = true;
      })
      .catch((err) => {
        console.warn("[terrain] snow material never ready, keeping fallback ground:", err);
      });
  }

  async finishBake(_onProgress?: (label: string, pct: number) => void): Promise<void> {
    // Height bake is owned by Heightfield; material already bound to its texture.
  }

  update(playerX: number, playerZ: number, deformation: DeformationField): void {
    this.lodX = playerX;
    this.lodZ = playerZ;
    this.snowMaterial?.update(deformation, playerX, playerZ);
    if (this.ground.isVisible) {
      this.ground.position.x = playerX;
      this.ground.position.z = playerZ;
      this.ground.position.y = this.heightfield.heightAt(playerX, playerZ) - 0.05;
    }
  }

  get material() {
    return this.snowMaterial;
  }

  dispose(): void {
    for (const ring of this.rings) ring.dispose();
    this.ground.dispose();
    this.snowMaterial?.dispose();
    this.root.dispose();
  }
}
