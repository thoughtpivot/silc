/**
 * Nested-ring clipmap as one static mesh.
 * Vertices store (gridI, ringLevel, gridJ) — world placement is entirely in the VS.
 */
import { BoundingInfo, Mesh, Scene, Vector3, VertexData } from "@babylonjs/core";

/** Quads per side per ring. Must be divisible by 4. */
export const GRID_N = 96;
export const LEVELS = 6;
/** Innermost vertex spacing, metres (~8.5 cm). */
export const BASE_SPACING = 0.085;

const HOLE_SHRINK = 3;
const HALF = GRID_N / 2;

export function buildClipmapMesh(scene: Scene): Mesh {
  const side = GRID_N + 1;
  const vertsPerLevel = side * side;
  const positions = new Float32Array(vertsPerLevel * LEVELS * 3);

  let quadCount = GRID_N * GRID_N;
  const holeHalf = HALF / 2 - HOLE_SHRINK;
  const holeQuads = holeHalf * 2 * (holeHalf * 2);
  quadCount += (LEVELS - 1) * (GRID_N * GRID_N - holeQuads);
  const indices = new Uint32Array(quadCount * 6);

  let vi = 0;
  let ii = 0;

  for (let level = 0; level < LEVELS; level++) {
    const vBase = level * vertsPerLevel;
    for (let j = 0; j <= GRID_N; j++) {
      const gj = j - HALF;
      for (let i = 0; i <= GRID_N; i++) {
        positions[vi++] = i - HALF;
        positions[vi++] = level;
        positions[vi++] = gj;
      }
    }

    for (let j = 0; j < GRID_N; j++) {
      const gj = j - HALF;
      for (let i = 0; i < GRID_N; i++) {
        const gi = i - HALF;
        if (level > 0) {
          const maxAbs = Math.max(
            Math.abs(gi),
            Math.abs(gi + 1),
            Math.abs(gj),
            Math.abs(gj + 1),
          );
          if (maxAbs <= holeHalf) continue;
        }
        const a = vBase + j * side + i;
        const b = a + 1;
        const c = a + side;
        const d = c + 1;
        if (((i + j) & 1) === 0) {
          indices[ii++] = a;
          indices[ii++] = b;
          indices[ii++] = c;
          indices[ii++] = b;
          indices[ii++] = d;
          indices[ii++] = c;
        } else {
          indices[ii++] = a;
          indices[ii++] = d;
          indices[ii++] = c;
          indices[ii++] = a;
          indices[ii++] = b;
          indices[ii++] = d;
        }
      }
    }
  }

  const mesh = new Mesh("terrainClipmap", scene);
  const vd = new VertexData();
  vd.positions = positions;
  vd.indices = ii === indices.length ? indices : indices.subarray(0, ii);
  vd.applyToMesh(mesh, false);
  mesh.alwaysSelectAsActiveMesh = true;
  mesh.isPickable = false;
  mesh.receiveShadows = false;
  // VS places verts far outside the grid AABB — keep a huge world bound.
  mesh.setBoundingInfo(
    new BoundingInfo(new Vector3(-2048, -64, -2048), new Vector3(2048, 256, 2048)),
  );
  mesh.doNotSyncBoundingInfo = true;
  mesh.freezeWorldMatrix();
  return mesh;
}

export const INNER_EXTENT = HALF * BASE_SPACING;
export const OUTER_EXTENT = HALF * BASE_SPACING * Math.pow(2, LEVELS - 1);
