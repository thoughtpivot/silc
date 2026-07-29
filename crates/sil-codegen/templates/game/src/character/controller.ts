import { MeshBuilder, Scene, TransformNode, Vector3 } from "@babylonjs/core";
import type { GameManifest } from "../manifest.ts";
import type { InputController } from "../controls/input.ts";
import type { SpringArmCamera } from "../camera/springArm.ts";
import type { Heightfield } from "../terrain/heightfield.ts";

export class CharacterController {
  readonly root: TransformNode;
  readonly position: Vector3;
  readonly velocity: Vector3;
  private readonly moveSpeed: number;
  private readonly heightfield: Heightfield;
  private footPhase = 0;
  private readonly footOffset = [
    { x: -0.12, z: 0.18 },
    { x: 0.12, z: -0.18 },
  ];
  private lastFootDown = [false, false];
  /** Planted foot world Y offsets for robe IK. */
  readonly footPlantY = [0, 0];
  onFootfall: ((x: number, z: number) => void) | null = null;

  constructor(scene: Scene, manifest: GameManifest, heightfield: Heightfield) {
    this.heightfield = heightfield;
    this.moveSpeed = manifest.character?.moveSpeed ?? 4.5;
    this.root = new TransformNode("characterRoot", scene);
    this.position = Vector3.Zero();
    this.velocity = Vector3.Zero();
    const collider = MeshBuilder.CreateBox(
      "charCollider",
      { width: 0.4, height: 1.7, depth: 0.35 },
      scene,
    );
    collider.parent = this.root;
    collider.isVisible = false;
    collider.isPickable = false;
  }

  groundY(x: number, z: number): number {
    return this.heightfield.heightAt(x, z);
  }

  update(dt: number, input: InputController, camera: SpringArmCamera, surfActive: boolean): void {
    if (surfActive) {
      this.velocity.scaleInPlace(0.95);
      this.root.rotation.y = camera.yaw;
      this.position.y = this.groundY(this.position.x, this.position.z);
      this.root.position.copyFrom(this.position);
      return;
    }

    const fwd = { x: 0, z: 0 };
    const right = { x: 0, z: 0 };
    camera.forwardXZ(fwd);
    camera.rightXZ(right);

    let mx = 0;
    let mz = 0;
    if (input.isDown("KeyW") || input.isDown("ArrowUp")) {
      mx += fwd.x;
      mz += fwd.z;
    }
    if (input.isDown("KeyS") || input.isDown("ArrowDown")) {
      mx -= fwd.x;
      mz -= fwd.z;
    }
    if (input.isDown("KeyA") || input.isDown("ArrowLeft")) {
      mx -= right.x;
      mz -= right.z;
    }
    if (input.isDown("KeyD") || input.isDown("ArrowRight")) {
      mx += right.x;
      mz += right.z;
    }

    const len = Math.sqrt(mx * mx + mz * mz);
    if (len > 0.001) {
      mx /= len;
      mz /= len;
    }

    const speed = this.moveSpeed;
    this.velocity.x = mx * speed;
    this.velocity.z = mz * speed;
    this.position.x += this.velocity.x * dt;
    this.position.z += this.velocity.z * dt;
    this.position.y = this.groundY(this.position.x, this.position.z);

    if (len > 0.01) {
      this.root.rotation.y = Math.atan2(mx, mz);
      this.footPhase += dt * 8;
      for (let f = 0; f < 2; f++) {
        const phase = this.footPhase + f * Math.PI;
        const down = Math.sin(phase) > 0.6;
        const fo = this.footOffset[f]!;
        const cy = Math.cos(this.root.rotation.y);
        const sy = Math.sin(this.root.rotation.y);
        const fx = this.position.x + fo.x * cy - fo.z * sy;
        const fz = this.position.z + fo.x * sy + fo.z * cy;
        // Plant: foot Y tracks ground; lift during swing.
        const lift = Math.max(0, Math.sin(phase)) * 0.12;
        this.footPlantY[f] = this.groundY(fx, fz) + lift;
        if (down && !this.lastFootDown[f]) {
          this.onFootfall?.(fx, fz);
        }
        this.lastFootDown[f] = down;
      }
    }

    this.root.position.copyFrom(this.position);
  }

  dispose(): void {
    this.root.dispose();
  }
}
