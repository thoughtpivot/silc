import { Matrix, Quaternion, Vector3 } from "@babylonjs/core/Maths/math.vector";

/** Module-scoped scratch objects — zero per-frame allocation in hot paths. */
export const scratchV3a = new Vector3();
export const scratchV3b = new Vector3();
export const scratchV3c = new Vector3();
export const scratchV3d = new Vector3();
export const scratchQuat = new Quaternion();
export const scratchMat = Matrix.Identity();

export class ObjectPool<T> {
  private readonly factory: () => T;
  private readonly reset: (item: T) => void;
  private readonly free: T[] = [];
  private readonly active: T[] = [];

  constructor(factory: () => T, reset: (item: T) => void, initial = 16) {
    this.factory = factory;
    this.reset = reset;
    for (let i = 0; i < initial; i++) {
      this.free.push(factory());
    }
  }

  acquire(): T {
    let item: T;
    if (this.free.length > 0) {
      item = this.free.pop()!;
    } else {
      item = this.factory();
    }
    this.active.push(item);
    return item;
  }

  release(item: T): void {
    const idx = this.active.indexOf(item);
    if (idx >= 0) {
      this.active[idx] = this.active[this.active.length - 1]!;
      this.active.pop();
    }
    this.reset(item);
    this.free.push(item);
  }

  releaseAll(): void {
    for (let i = this.active.length - 1; i >= 0; i--) {
      this.reset(this.active[i]!);
      this.free.push(this.active[i]!);
    }
    this.active.length = 0;
  }

  get activeCount(): number {
    return this.active.length;
  }
}

export type PooledParticle = {
  x: number;
  y: number;
  z: number;
  vx: number;
  vy: number;
  vz: number;
  life: number;
  maxLife: number;
  size: number;
  active: boolean;
};

function resetParticle(p: PooledParticle): void {
  p.x = 0;
  p.y = 0;
  p.z = 0;
  p.vx = 0;
  p.vy = 0;
  p.vz = 0;
  p.life = 0;
  p.maxLife = 1;
  p.size = 0.05;
  p.active = false;
}

export const particlePool = new ObjectPool<PooledParticle>(
  () => ({ x: 0, y: 0, z: 0, vx: 0, vy: 0, vz: 0, life: 0, maxLife: 1, size: 0.05, active: false }),
  resetParticle,
  512,
);

export type RibbonPoint = {
  position: Vector3;
  width: number;
  t: number;
};

export class RibbonBuffer {
  readonly points: RibbonPoint[] = [];
  readonly maxPoints: number;
  head = 0;
  count = 0;

  constructor(maxPoints: number) {
    this.maxPoints = maxPoints;
    for (let i = 0; i < maxPoints; i++) {
      this.points.push({ position: Vector3.Zero(), width: 0.2, t: 0 });
    }
  }

  push(pos: Vector3, width: number, t: number): void {
    const idx = (this.head + this.count) % this.maxPoints;
    const p = this.points[idx]!;
    p.position.copyFrom(pos);
    p.width = width;
    p.t = t;
    if (this.count < this.maxPoints) {
      this.count++;
    } else {
      this.head = (this.head + 1) % this.maxPoints;
    }
  }

  clear(): void {
    this.head = 0;
    this.count = 0;
  }
}
