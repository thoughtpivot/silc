import type { AbstractEngine } from "@babylonjs/core";
import type { GameManifest } from "../manifest.ts";
import type { PostChainHandle, PostStageName } from "../post/chain.ts";
import type { DeformationField } from "../terrain/deformation.ts";
import type { AtmosphereHandle } from "../environment/atmosphere.ts";
import type { InputController } from "../controls/input.ts";

const FRAME_HISTORY = 120;

export class SettingsOverlay {
  visible = false;
  private readonly root: HTMLDivElement;
  private readonly canvas: HTMLCanvasElement;
  private readonly ctx: CanvasRenderingContext2D;
  private readonly statsEl: HTMLDivElement;
  private readonly togglesEl: HTMLDivElement;
  private readonly frameTimes: Float32Array;
  private frameIdx = 0;
  private frameCount = 0;
  private lastToggle = false;
  private updateTimer = 0;
  private textBuf = "";

  constructor(
    private readonly manifest: GameManifest,
    private readonly post: PostChainHandle,
    private readonly deformation: DeformationField,
    private readonly atmosphere: AtmosphereHandle,
  ) {
    this.frameTimes = new Float32Array(FRAME_HISTORY);

    this.root = document.createElement("div");
    this.root.id = "settings-overlay";
    Object.assign(this.root.style, {
      position: "fixed",
      top: "12px",
      left: "12px",
      width: "320px",
      padding: "12px",
      background: "rgba(8, 12, 20, 0.88)",
      color: "#c8d8f0",
      fontFamily: "monospace",
      fontSize: "11px",
      borderRadius: "6px",
      display: "none",
      zIndex: "200",
      pointerEvents: "auto",
    });

    this.canvas = document.createElement("canvas");
    this.canvas.width = 280;
    this.canvas.height = 60;
    this.ctx = this.canvas.getContext("2d")!;

    this.statsEl = document.createElement("div");
    this.togglesEl = document.createElement("div");
    this.root.appendChild(this.canvas);
    this.root.appendChild(this.statsEl);
    this.root.appendChild(this.togglesEl);
    document.body.appendChild(this.root);

    this.buildToggles();
  }

  attach(input: InputController): void {
    void input;
  }

  private buildToggles(): void {
    const stages: PostStageName[] = ["taa", "ssao", "ssr", "dof", "bloom", "tonemap", "grain", "sharpen"];
    for (let i = 0; i < stages.length; i++) {
      const stage = stages[i]!;
      const label = document.createElement("label");
      label.style.display = "block";
      const cb = document.createElement("input");
      cb.type = "checkbox";
      cb.checked = this.post.toggles[stage];
      cb.addEventListener("change", () => {
        this.post.setEnabled(stage, cb.checked);
      });
      label.appendChild(cb);
      label.appendChild(document.createTextNode(` ${this.post.labels[stage] ?? stage}`));
      if (stage === "ssr") {
        cb.disabled = true;
        label.title = "SSR not available in this runtime — catalog toggle retained for parity";
      }
      this.togglesEl.appendChild(label);
    }

    const sunLabel = document.createElement("label");
    sunLabel.style.display = "block";
    sunLabel.style.marginTop = "8px";
    const sunSlider = document.createElement("input");
    sunSlider.type = "range";
    sunSlider.min = "5";
    sunSlider.max = "45";
    sunSlider.value = String(this.manifest.environment?.sunElevationDeg ?? 12);
    sunSlider.addEventListener("input", () => {
      this.atmosphere.sun.direction.y = -Math.sin((Number(sunSlider.value) * Math.PI) / 180);
    });
    sunLabel.appendChild(document.createTextNode(" Sun° "));
    sunLabel.appendChild(sunSlider);
    this.togglesEl.appendChild(sunLabel);
  }

  tick(input: InputController, engine: AbstractEngine, sceneTris: number, drawCalls: number): void {
    const pressed = input.overlayTogglePressed();
    if (pressed && !this.lastToggle) {
      this.visible = !this.visible;
      this.root.style.display = this.visible ? "block" : "none";
    }
    this.lastToggle = pressed;

    const dt = engine.getDeltaTime();
    this.frameTimes[this.frameIdx % FRAME_HISTORY] = dt;
    this.frameIdx++;

    if (!this.visible) return;

    this.updateTimer += dt;
    if (this.updateTimer < 0.25) return;
    this.updateTimer = 0;

    this.drawGraph();
    const fps = (1 / dt).toFixed(0);
    let sorted = 0;
    const copy = new Float32Array(FRAME_HISTORY);
    for (let i = 0; i < FRAME_HISTORY; i++) copy[i] = this.frameTimes[i]!;
    copy.sort();
    sorted = copy[Math.floor(FRAME_HISTORY * 0.99)]!;
    const onePct = sorted > 0 ? (1 / sorted).toFixed(0) : "—";

    this.textBuf = `FPS ${fps} · 1% low ${onePct}\n`;
    this.textBuf += `Draw ${drawCalls} · Tris ${sceneTris}\n`;
    this.textBuf += `Deform ${this.deformation.resolution}²`;
    this.statsEl.textContent = this.textBuf;
  }

  private drawGraph(): void {
    const w = this.canvas.width;
    const h = this.canvas.height;
    this.ctx.fillStyle = "#0a1420";
    this.ctx.fillRect(0, 0, w, h);
    this.ctx.strokeStyle = "#6090c0";
    this.ctx.beginPath();
    for (let i = 0; i < FRAME_HISTORY; i++) {
      const idx = (this.frameIdx - FRAME_HISTORY + i + FRAME_HISTORY * 2) % FRAME_HISTORY;
      const t = this.frameTimes[idx]!;
      const ms = Math.min(t, 0.033) / 0.033;
      const x = (i / FRAME_HISTORY) * w;
      const y = h - ms * h;
      if (i === 0) this.ctx.moveTo(x, y);
      else this.ctx.lineTo(x, y);
    }
    this.ctx.stroke();
    this.ctx.strokeStyle = "rgba(255,80,80,0.5)";
    const budgetY = h - (1 / 90 / 0.033) * h;
    this.ctx.beginPath();
    this.ctx.moveTo(0, budgetY);
    this.ctx.lineTo(w, budgetY);
    this.ctx.stroke();
  }

  dispose(): void {
    this.root.remove();
  }
}
