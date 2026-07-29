import {
  Color3,
  Mesh,
  MeshBuilder,
  Scene,
  StandardMaterial,
  TransformNode,
  Vector3,
} from "@babylonjs/core";
import type { GameManifest } from "../manifest.ts";

type ClothParticle = {
  x: number;
  y: number;
  z: number;
  px: number;
  py: number;
  pz: number;
  pinned: boolean;
};

export class RobeFigure {
  readonly root: TransformNode;
  readonly bodyMesh: Mesh;
  private readonly mantleMesh: Mesh;
  private readonly sleeveL: Mesh;
  private readonly sleeveR: Mesh;
  private readonly hemParticles: ClothParticle[] = [];
  private readonly wind = new Vector3(0.35, 0, 0.18);
  private furShells: Mesh[] = [];
  private readonly footSpray: Mesh;

  constructor(scene: Scene, manifest: GameManifest, parent: TransformNode) {
    this.root = new TransformNode("robeRoot", scene);
    this.root.parent = parent;

    this.bodyMesh = this.buildBody(scene);
    this.mantleMesh = this.buildMantle(scene);
    this.sleeveL = this.buildSleeve(scene, -1);
    this.sleeveR = this.buildSleeve(scene, 1);
    this.footSpray = MeshBuilder.CreateSphere("footSpray", { diameter: 0.35, segments: 6 }, scene);
    this.footSpray.parent = this.root;
    this.footSpray.position.y = 0.05;
    this.footSpray.isVisible = false;
    const sprayMat = new StandardMaterial("footSprayMat", scene);
    sprayMat.diffuseColor = new Color3(0.9, 0.94, 1);
    sprayMat.alpha = 0.35;
    sprayMat.backFaceCulling = false;
    sprayMat.freeze();
    this.footSpray.material = sprayMat;

    if (manifest.character?.cloth !== false) {
      this.initCloth(manifest.character?.clothRegions ?? ["hem", "mantle"]);
    }
    if (manifest.character?.fur !== false) {
      const shells = Math.max(
        8,
        Math.min(40, Math.round(manifest.character?.furShells ?? 24)),
      );
      this.furShells = this.buildFurShells(scene, shells, manifest.character?.furRegions ?? ["hood"]);
    }
  }

  private buildBody(scene: Scene): Mesh {
    const torso = MeshBuilder.CreateCylinder(
      "robeTorso",
      { height: 1.05, diameterTop: 0.34, diameterBottom: 0.62, tessellation: 28 },
      scene,
    );
    torso.position.y = 1.05;
    const chest = MeshBuilder.CreateSphere("robeChest", { diameter: 0.52, segments: 14 }, scene);
    chest.position.y = 1.35;
    chest.scaling.set(1.05, 0.7, 0.75);
    const skirt = MeshBuilder.CreateCylinder(
      "robeSkirt",
      { height: 0.7, diameterTop: 0.58, diameterBottom: 1.05, tessellation: 28 },
      scene,
    );
    skirt.position.y = 0.32;
    const hood = MeshBuilder.CreateSphere("hood", { diameter: 0.52, slice: 0.58, segments: 18 }, scene);
    hood.position.y = 1.68;
    hood.rotation.x = Math.PI;
    const hoodRim = MeshBuilder.CreateTorus(
      "hoodRim",
      { diameter: 0.44, thickness: 0.08, tessellation: 20 },
      scene,
    );
    hoodRim.position.y = 1.55;
    hoodRim.rotation.x = Math.PI / 2;
    const face = MeshBuilder.CreateSphere("faceHint", { diameter: 0.2, segments: 12 }, scene);
    face.position.set(0, 1.58, 0.12);
    const faceMat = new StandardMaterial("faceMat", scene);
    faceMat.diffuseColor = new Color3(0.55, 0.38, 0.28);
    faceMat.emissiveColor = new Color3(0.18, 0.1, 0.06);
    faceMat.specularColor = new Color3(0.35, 0.25, 0.15);
    face.material = faceMat;
    const merged = Mesh.MergeMeshes([torso, chest, skirt, hood, hoodRim], true, true)!;
    merged.name = "robeBody";
    merged.parent = this.root;
    const mat = new StandardMaterial("robeMat", scene);
    mat.diffuseColor = new Color3(0.1, 0.12, 0.2);
    mat.specularColor = new Color3(0.06, 0.06, 0.08);
    mat.emissiveColor = new Color3(0.035, 0.04, 0.065);
    mat.freeze();
    merged.material = mat;
    face.parent = this.root;
    return merged;
  }

  private buildMantle(scene: Scene): Mesh {
    const mantle = MeshBuilder.CreateCylinder(
      "mantle",
      { height: 0.12, diameterTop: 0.85, diameterBottom: 1.25, tessellation: 32 },
      scene,
    );
    mantle.position.y = 0.78;
    mantle.parent = this.root;
    const mat = new StandardMaterial("mantleMat", scene);
    mat.diffuseColor = new Color3(0.1, 0.12, 0.18);
    mat.emissiveColor = new Color3(0.03, 0.035, 0.05);
    mat.backFaceCulling = false;
    mat.freeze();
    mantle.material = mat;
    return mantle;
  }

  private buildSleeve(scene: Scene, side: number): Mesh {
    const sleeve = MeshBuilder.CreateCylinder(
      `sleeve${side}`,
      { height: 0.72, diameterTop: 0.16, diameterBottom: 0.28, tessellation: 14 },
      scene,
    );
    sleeve.position.set(side * 0.38, 1.15, 0.02);
    sleeve.rotation.z = side * 0.85;
    sleeve.parent = this.root;
    const mat = new StandardMaterial(`sleeveMat${side}`, scene);
    mat.diffuseColor = new Color3(0.11, 0.13, 0.2);
    mat.emissiveColor = new Color3(0.03, 0.035, 0.055);
    mat.freeze();
    sleeve.material = mat;
    return sleeve;
  }

  private initCloth(regions: string[]): void {
    const density = regions.includes("sleeves") ? 44 : 36;
    for (let i = 0; i < density; i++) {
      const t = i / (density - 1);
      const radius = regions.includes("mantle") ? 0.58 : 0.5;
      const y = regions.includes("hem") ? 0.42 : 0.7;
      this.hemParticles.push({
        x: Math.sin(t * Math.PI * 2) * radius,
        y,
        z: Math.cos(t * Math.PI * 2) * radius,
        px: Math.sin(t * Math.PI * 2) * radius,
        py: y,
        pz: Math.cos(t * Math.PI * 2) * radius,
        pinned: i % 7 === 0,
      });
    }
  }

  private buildFurShells(scene: Scene, count: number, regions: string[]): Mesh[] {
    const shells: Mesh[] = [];
    if (regions.includes("hood") || regions.length === 0) {
      const base = MeshBuilder.CreateSphere(
        "furBase",
        { diameter: 0.46, slice: 0.48, segments: 12 },
        scene,
      );
      base.position.y = 1.55;
      base.parent = this.root;
      base.isVisible = false;

      for (let s = 0; s < count; s++) {
        const t = s / count;
        const shell = base.clone(`furShell${s}`)!;
        shell.isVisible = true;
        shell.scaling.setAll(1 + t * 0.1);
        const mat = new StandardMaterial(`furMat${s}`, scene);
        mat.diffuseColor = new Color3(0.22 + t * 0.08, 0.2 + t * 0.05, 0.24);
        mat.emissiveColor = new Color3(0.04, 0.04, 0.05);
        mat.alpha = 0.18 + t * 0.04;
        mat.backFaceCulling = false;
        mat.freeze();
        shell.material = mat;
        shells.push(shell);
      }
    }
    if (regions.includes("cuffs")) {
      for (const side of [-1, 1]) {
        for (let s = 0; s < Math.min(8, count / 3); s++) {
          const t = s / 8;
          const cuff = MeshBuilder.CreateTorus(
            `furCuff${side}_${s}`,
            { diameter: 0.18 + t * 0.04, thickness: 0.04, tessellation: 10 },
            scene,
          );
          cuff.position.set(side * 0.32, 0.95 - t * 0.05, 0.02);
          cuff.rotation.z = side * 0.55;
          cuff.parent = this.root;
          const mat = new StandardMaterial(`furCuffMat${side}_${s}`, scene);
          mat.diffuseColor = new Color3(0.2, 0.19, 0.22);
          mat.alpha = 0.2;
          mat.backFaceCulling = false;
          mat.freeze();
          cuff.material = mat;
          shells.push(cuff);
        }
      }
    }
    return shells;
  }

  pulseFootSpray(): void {
    this.footSpray.isVisible = true;
    this.footSpray.scaling.setAll(0.4);
  }

  update(dt: number, velocity: Vector3, surfActive: boolean): void {
    const windScale = surfActive ? 5.5 : 1.15;
    const speed = Math.hypot(velocity.x, velocity.z);
    const n = this.hemParticles.length;

    // Verlet integration with distance constraints along the hem ring.
    for (let i = 0; i < n; i++) {
      const p = this.hemParticles[i]!;
      if (p.pinned) continue;
      const ox = p.x;
      const oy = p.y;
      const oz = p.z;
      const ax = (p.x - p.px) * 0.96 + velocity.x * -0.12 * windScale * dt;
      const ay = (p.y - p.py) * 0.96 - 1.8 * dt * dt;
      const az = (p.z - p.pz) * 0.96 + velocity.z * -0.12 * windScale * dt;
      p.px = ox;
      p.py = oy;
      p.pz = oz;
      p.x = ox + ax + this.wind.x * windScale * dt * 0.4;
      p.y = Math.max(0.28, oy + ay);
      p.z = oz + az + this.wind.z * windScale * dt * 0.4;
    }
    for (let iter = 0; iter < 2; iter++) {
      for (let i = 0; i < n; i++) {
        const a = this.hemParticles[i]!;
        const b = this.hemParticles[(i + 1) % n]!;
        if (a.pinned && b.pinned) continue;
        const dx = b.x - a.x;
        const dy = b.y - a.y;
        const dz = b.z - a.z;
        const dist = Math.hypot(dx, dy, dz) || 0.001;
        const rest = (Math.PI * 2 * 0.55) / Math.max(1, n);
        const corr = (dist - rest) / dist * 0.35;
        if (!a.pinned) {
          a.x += dx * corr;
          a.y += dy * corr;
          a.z += dz * corr;
        }
        if (!b.pinned) {
          b.x -= dx * corr;
          b.y -= dy * corr;
          b.z -= dz * corr;
        }
      }
    }

    if (n > 0) {
      let swayX = 0;
      let swayZ = 0;
      for (let i = 0; i < n; i++) {
        swayX += this.hemParticles[i]!.x;
        swayZ += this.hemParticles[i]!.z;
      }
      this.mantleMesh.rotation.y = (swayX / n) * 0.1;
      this.mantleMesh.rotation.x = Math.min(0.35, speed * 0.04 + (surfActive ? 0.2 : 0));
      this.mantleMesh.rotation.z = (swayZ / n) * 0.06 + velocity.x * 0.025;
    }

    const sleeveSwing = Math.sin(performance.now() * 0.004 + speed) * 0.1 * Math.min(1, speed + 0.2);
    this.sleeveL.rotation.x = sleeveSwing + (surfActive ? 0.35 : 0);
    this.sleeveR.rotation.x = -sleeveSwing + (surfActive ? 0.35 : 0);
    this.sleeveL.rotation.z = -0.55 - (surfActive ? 0.25 : 0);
    this.sleeveR.rotation.z = 0.55 + (surfActive ? 0.25 : 0);

    // Fur shells slightly trail velocity for secondary motion.
    for (let s = 0; s < this.furShells.length; s++) {
      const shell = this.furShells[s]!;
      if (shell.name.startsWith("furShell")) {
        shell.rotation.x = -velocity.z * 0.01 * (s / Math.max(1, this.furShells.length));
        shell.rotation.z = velocity.x * 0.01 * (s / Math.max(1, this.furShells.length));
      }
    }

    if (this.footSpray.isVisible) {
      const s = this.footSpray.scaling.x + dt * 4;
      if (s > 1.8) {
        this.footSpray.isVisible = false;
      } else {
        this.footSpray.scaling.setAll(s);
      }
    }

    if (surfActive) {
      this.root.rotation.x = -0.32;
      this.root.rotation.z *= 0.9;
    } else {
      this.root.rotation.x *= 0.88;
    }
  }

  dispose(): void {
    for (const s of this.furShells) s.dispose();
    this.footSpray.dispose();
    this.sleeveL.dispose();
    this.sleeveR.dispose();
    this.mantleMesh.dispose();
    this.bodyMesh.dispose();
    this.root.dispose();
  }
}
