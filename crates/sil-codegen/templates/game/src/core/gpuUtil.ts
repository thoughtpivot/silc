/** Wait for Babylon async shader compile / texture readiness. */

export function whenReady(
  obj: { isReady: (...args: unknown[]) => boolean },
  label: string,
  args: unknown[] = [],
  timeoutMs = 25000,
): Promise<void> {
  return new Promise((resolve, reject) => {
    const t0 = performance.now();
    const tick = (): void => {
      let ok = false;
      try {
        ok = obj.isReady(...args);
      } catch (e) {
        reject(new Error(`${label} isReady() threw: ${(e as Error).message}`));
        return;
      }
      if (ok) {
        resolve();
        return;
      }
      if (performance.now() - t0 > timeoutMs) {
        reject(
          new Error(
            `${label} never became ready after ${timeoutMs}ms — usually a WGSL compile error`,
          ),
        );
        return;
      }
      setTimeout(tick, 0);
    };
    tick();
  });
}

export async function bakeOnce(
  pt: { isReady: (...args: unknown[]) => boolean; render: () => void; name?: string },
  label?: string,
): Promise<void> {
  await whenReady(pt, label || pt.name || "bake");
  pt.render();
}
