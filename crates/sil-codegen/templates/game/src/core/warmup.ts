import type { GameEngine } from "./engine.ts";

export type WarmupProgress = (label: string, pct: number) => void;

/** Short warm-up — never block on rAF (background tabs throttle it to ~0). */
export async function runWarmup(game: GameEngine, onProgress: WarmupProgress): Promise<void> {
  const scene = game.scene;
  onProgress("Warming pipelines", 0.85);
  for (let i = 0; i < 3; i++) {
    game.update(1 / 60);
    scene.render();
    await new Promise((r) => setTimeout(r, 0));
  }
  game.water.mesh.isVisible = false;
  onProgress("Ready", 1);
}
