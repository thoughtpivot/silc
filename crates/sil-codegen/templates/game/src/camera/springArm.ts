import type { ArcRotateCamera } from "@babylonjs/core";
import type { GameManifest } from "../manifest.ts";
import { scratchV3a, scratchV3b } from "../core/pools.ts";

export class SpringArmCamera {
  readonly baseFov: number;
  private readonly cam: ArcRotateCamera;
  private readonly shoulderOffsetM: number;
  private fovCurrent: number;
  private radiusCurrent: number;
  private alphaCurrent: number;
  private betaCurrent: number;
  private impulseX = 0;
  private impulseY = 0;
  /** Cached camera-relative XZ basis (recomputed each update). */
  private fwdX = 0;
  private fwdZ = 1;
  private rightX = 1;
  private rightZ = 0;

  constructor(cam: ArcRotateCamera, manifest: GameManifest) {
    this.cam = cam;
    this.baseFov = manifest.camera?.fovDeg ?? 55;
    this.shoulderOffsetM = manifest.camera?.shoulderOffsetM ?? 0.45;
    this.fovCurrent = this.baseFov;
    this.radiusCurrent = Math.max(manifest.camera?.distanceM ?? 5.5, 11);
    this.alphaCurrent = cam.alpha;
    this.betaCurrent = Math.max(cam.beta, 1.05);
    cam.fov = (this.baseFov * Math.PI) / 180;
    this.rebuildBasis();
  }

  /** Debug / CDP gate framing — bypass look-smoothing. */
  setFraming(alpha: number, beta: number, radius: number): void {
    this.alphaCurrent = alpha;
    this.betaCurrent = Math.max(0.4, Math.min(1.42, beta));
    this.radiusCurrent = Math.max(2.5, Math.min(36, radius));
    this.cam.alpha = this.alphaCurrent;
    this.cam.beta = this.betaCurrent;
    this.cam.radius = this.radiusCurrent;
    this.rebuildBasis();
  }

  get yaw(): number {
    // Facing yaw matching screen-forward (into the view).
    return Math.atan2(this.fwdX, this.fwdZ);
  }

  private rebuildBasis(): void {
    // Derive from the live camera so WASD matches what is on screen.
    // Look vector = target - eye; flatten to XZ for ground movement.
    const eye = this.cam.position;
    const target = this.cam.getTarget();
    let fx = target.x - eye.x;
    let fz = target.z - eye.z;
    const len = Math.hypot(fx, fz);
    if (len < 1e-5) {
      fx = -Math.sin(this.alphaCurrent);
      fz = -Math.cos(this.alphaCurrent);
    } else {
      fx /= len;
      fz /= len;
    }
    this.fwdX = fx;
    this.fwdZ = fz;
    // Babylon is left-handed, Y-up: right = up × forward → (fz, -fx)
    this.rightX = fz;
    this.rightZ = -fx;
  }

  update(
    dt: number,
    target: { x: number; y: number; z: number },
    mouse: { dx: number; dy: number; wheel: number },
    speed: number,
  ): void {
    this.alphaCurrent -= mouse.dx * 0.0055;
    this.betaCurrent = Math.max(0.4, Math.min(1.42, this.betaCurrent + mouse.dy * 0.0055));

    this.radiusCurrent += mouse.wheel * 0.008;
    this.radiusCurrent = Math.max(2.5, Math.min(36, this.radiusCurrent));

    const targetFov = this.baseFov + speed * 8;
    this.fovCurrent += (targetFov - this.fovCurrent) * 0.06;
    this.cam.fov = (this.fovCurrent * Math.PI) / 180;

    const shoulder = this.shoulderOffsetM;
    scratchV3a.x = Math.sin(this.alphaCurrent) * shoulder;
    scratchV3a.y = 0;
    scratchV3a.z = Math.cos(this.alphaCurrent) * shoulder;

    this.impulseX *= 0.92;
    this.impulseY *= 0.92;

    const ease = 1 - Math.pow(0.001, dt);
    this.cam.alpha += (this.alphaCurrent - this.cam.alpha) * ease;
    this.cam.beta += (this.betaCurrent - this.cam.beta) * ease;
    this.cam.radius += (this.radiusCurrent - this.cam.radius) * ease;

    scratchV3b.x = target.x + scratchV3a.x + this.impulseX;
    scratchV3b.y = target.y + 1.4;
    scratchV3b.z = target.z + scratchV3a.z + this.impulseY;
    this.cam.target.copyFrom(scratchV3b);
    this.rebuildBasis();
  }

  applyImpulse(strength: number): void {
    this.impulseX += (Math.random() - 0.5) * strength;
    this.impulseY += (Math.random() - 0.5) * strength * 0.5;
  }

  forwardXZ(out: { x: number; z: number }): void {
    out.x = this.fwdX;
    out.z = this.fwdZ;
  }

  rightXZ(out: { x: number; z: number }): void {
    out.x = this.rightX;
    out.z = this.rightZ;
  }
}
