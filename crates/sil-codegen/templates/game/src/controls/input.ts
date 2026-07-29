import type { GameManifest } from "../manifest.ts";

export class InputController {
  readonly keys = new Set<string>();
  mouseDx = 0;
  mouseDy = 0;
  wheelDelta = 0;
  holdRmb = false;
  pointerLocked = false;

  private readonly manifest: GameManifest;
  private canvas: HTMLCanvasElement | null = null;
  private bound = false;
  private holdLmb = false;
  private looking = false;

  constructor(manifest: GameManifest) {
    this.manifest = manifest;
  }

  attach(canvas: HTMLCanvasElement): void {
    if (this.bound) return;
    this.canvas = canvas;
    this.bound = true;
    canvas.tabIndex = 0;
    canvas.style.outline = "none";
    canvas.style.cursor = "grab";
    canvas.style.touchAction = "none";

    window.addEventListener("keydown", this.onKeyDown);
    window.addEventListener("keyup", this.onKeyUp);
    window.addEventListener("blur", this.onBlur);
    // Document-level move/up so drag-look keeps working if the cursor leaves the canvas.
    document.addEventListener("pointerlockchange", this.onPointerLockChange);
    document.addEventListener("mousemove", this.onMouseMove);
    document.addEventListener("mouseup", this.onMouseUp);
    canvas.addEventListener("pointerdown", this.onPointerDown);
    canvas.addEventListener("wheel", this.onWheel, { passive: false });
    canvas.addEventListener("contextmenu", (e) => e.preventDefault());
    canvas.addEventListener("click", this.onClick);
  }

  private onClick = (): void => {
    this.canvas?.focus();
    if (this.canvas && document.pointerLockElement !== this.canvas) {
      void this.canvas.requestPointerLock();
    }
  };

  private onPointerLockChange = (): void => {
    this.pointerLocked = document.pointerLockElement === this.canvas;
    if (this.canvas) {
      this.canvas.style.cursor = this.pointerLocked ? "none" : this.looking ? "grabbing" : "grab";
    }
  };

  private onKeyDown = (e: KeyboardEvent): void => {
    this.keys.add(e.code);
    if (
      e.code === "KeyW" ||
      e.code === "KeyA" ||
      e.code === "KeyS" ||
      e.code === "KeyD" ||
      e.code === "ArrowUp" ||
      e.code === "ArrowDown" ||
      e.code === "ArrowLeft" ||
      e.code === "ArrowRight" ||
      e.code === "Escape"
    ) {
      e.preventDefault();
    }
  };

  private onKeyUp = (e: KeyboardEvent): void => {
    this.keys.delete(e.code);
  };

  private onBlur = (): void => {
    this.keys.clear();
    this.holdRmb = false;
    this.holdLmb = false;
    this.looking = false;
  };

  private onPointerDown = (e: PointerEvent): void => {
    if (!this.canvas) return;
    this.canvas.focus();

    if (e.button === 2) {
      this.holdRmb = true;
      e.preventDefault();
      return;
    }

    if (e.button === 0 || e.button === 1) {
      this.holdLmb = e.button === 0;
      this.looking = true;
      this.canvas.style.cursor = "grabbing";
      try {
        this.canvas.setPointerCapture(e.pointerId);
      } catch {
        /* ignore */
      }
      if (document.pointerLockElement !== this.canvas) {
        void this.canvas.requestPointerLock();
      }
      e.preventDefault();
    }
  };

  private onMouseUp = (e: MouseEvent): void => {
    if (e.button === 2) this.holdRmb = false;
    if (e.button === 0 || e.button === 1) {
      this.holdLmb = false;
      if (!this.pointerLocked) {
        this.looking = false;
        if (this.canvas) this.canvas.style.cursor = "grab";
      }
    }
  };

  private onMouseMove = (e: MouseEvent): void => {
    const locked = document.pointerLockElement === this.canvas;
    this.pointerLocked = locked;
    // Look when locked, or while dragging LMB/MMB (fallback if lock is denied).
    if (locked || this.looking || this.holdLmb || (e.buttons & 1) !== 0 || (e.buttons & 4) !== 0) {
      this.mouseDx += e.movementX;
      this.mouseDy += e.movementY;
    }
  };

  private onWheel = (e: WheelEvent): void => {
    this.wheelDelta += e.deltaY;
    e.preventDefault();
  };

  update(): void {
    void this.manifest;
  }

  isDown(code: string): boolean {
    return this.keys.has(code);
  }

  consumeMouse(): { dx: number; dy: number; wheel: number } {
    const dx = this.mouseDx;
    const dy = this.mouseDy;
    const wheel = this.wheelDelta;
    this.mouseDx = 0;
    this.mouseDy = 0;
    this.wheelDelta = 0;
    return { dx, dy, wheel };
  }

  abilityKeyPressed(): string | null {
    const codes = ["Digit1", "Digit2", "Digit3", "Digit4", "Digit5"];
    for (let i = 0; i < codes.length; i++) {
      if (this.keys.has(codes[i]!)) return String(i + 1);
    }
    return null;
  }

  overlayTogglePressed(): boolean {
    const toggle = this.manifest.overlay?.toggle ?? "F1";
    const code = toggle === "F1" ? "F1" : toggle === "Backquote" ? "Backquote" : `Key${toggle}`;
    return this.keys.has(code);
  }

  dispose(): void {
    window.removeEventListener("keydown", this.onKeyDown);
    window.removeEventListener("keyup", this.onKeyUp);
    window.removeEventListener("blur", this.onBlur);
    document.removeEventListener("pointerlockchange", this.onPointerLockChange);
    document.removeEventListener("mousemove", this.onMouseMove);
    document.removeEventListener("mouseup", this.onMouseUp);
    if (this.canvas) {
      this.canvas.removeEventListener("pointerdown", this.onPointerDown);
      this.canvas.removeEventListener("wheel", this.onWheel);
      this.canvas.removeEventListener("click", this.onClick);
    }
  }
}
