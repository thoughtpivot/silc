import { loadManifest } from "./manifest.ts";
import { createGameEngine, startRenderLoop } from "./core/engine.ts";
import { runWarmup } from "./core/warmup.ts";

function showNoWebGpu(detail?: string): void {
  const el = document.getElementById("no-webgpu");
  const loading = document.getElementById("loading");
  if (loading) loading.style.display = "none";
  if (el) {
    el.style.display = "flex";
    if (detail) el.textContent = `WebGPU is required. ${detail}`;
  }
}

function setLoadingProgress(label: string, pct: number): void {
  const fill = document.getElementById("loading-bar-fill");
  const status = document.getElementById("loading-status");
  const title = document.getElementById("loading-title");
  if (fill) fill.style.width = `${Math.round(pct * 100)}%`;
  if (status) status.textContent = label;
  if (title && pct >= 1) title.textContent = "";
}

function hideLoading(): void {
  const loading = document.getElementById("loading");
  if (loading) {
    loading.classList.add("hidden");
    setTimeout(() => loading.remove(), 500);
  }
}

async function boot(): Promise<void> {
  setLoadingProgress("Boot starting", 0.02);
  if (!navigator.gpu) {
    showNoWebGpu("navigator.gpu is missing in this browser.");
    return;
  }

  const canvas = document.getElementById("game-root") as HTMLCanvasElement | null;
  if (!canvas) throw new Error("Missing #game-root canvas");

  setLoadingProgress("Loading manifest", 0.05);
  const manifest = await loadManifest();
  if (manifest.renderer !== "webgpu") {
    showNoWebGpu(`renderer=${String(manifest.renderer)}`);
    return;
  }

  try {
    const bakeRes = await fetch("/baked/bake.json");
    if (bakeRes.ok) {
      (manifest as { baked?: unknown }).baked = await bakeRes.json();
    }
  } catch {
    /* procedural fallback */
  }

  document.title = manifest.title;
  const titleEl = document.getElementById("loading-title");
  if (titleEl) titleEl.textContent = manifest.title;

  setLoadingProgress("Starting WebGPU", 0.1);
  const game = await createGameEngine(canvas, manifest, setLoadingProgress);

  game.character.onFootfall = (x, z) => {
    game.deformation.brush(x, z, 0.18, 0.06, { compression: 0.35 });
    game.robe.pulseFootSpray();
  };

  await runWarmup(game, setLoadingProgress);

  game.engine.resize();
  game.scene.render();
  hideLoading();

  (window as unknown as { __silcGame?: typeof game }).__silcGame = game;

  canvas.focus();
  startRenderLoop(game, manifest.targetFps);
  window.addEventListener("resize", () => {
    // Defer one frame so we never resize mid-submit (IOSurface destroy hazard).
    requestAnimationFrame(() => game.engine.resize());
  });
}

boot().catch((err) => {
  console.error(err);
  const status = document.getElementById("loading-status");
  if (status) {
    status.textContent = String(err);
    status.style.opacity = "1";
    status.style.color = "#f0a0a0";
    status.style.maxWidth = "32rem";
    status.style.textAlign = "center";
    status.style.whiteSpace = "pre-wrap";
  }
});
