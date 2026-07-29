/** Lowered game manifest emitted by silc game_lower from `game::scene` trees. */

export type EffectNode = {
  type: string;
  props?: Record<string, number | string | boolean>;
  children?: EffectNode[];
};

export type HeightLayerKind = "dune" | "drift" | "sastrugi" | "ripple";

export type HeightLayer = {
  kind: string;
  scaleM: number;
  amplitudeM: number;
  shear?: number;
};

export type PostStage = {
  name: string;
  enabled: boolean;
};

export type MovementMode = {
  name: string;
  hold: string;
  effects: EffectNode[];
};

export type AbilityDef = {
  name: string;
  key: string;
  effects: EffectNode[];
};

export type DynamicLightDef = {
  radiusM: number;
  intensity: number;
  color: string;
};

export type GameManifest = {
  title: string;
  targetFps: number;
  renderer: "webgpu";
  terrain?: {
    windDeg: number;
    layers: HeightLayer[];
    extentM?: number;
    nearSpacingCm?: number;
  };
  surface?: { profile: string; glint?: number; scatter?: number };
  deformation?: {
    extentM: number;
    texelCm: number;
    resolution?: number;
    refillRate?: number;
  };
  environment?: {
    sunElevationDeg: number;
    sunAzimuthDeg?: number;
    fogDensity: number;
    spindrift?: boolean;
  };
  post?: PostStage[];
  character?: {
    style?: string;
    robe: boolean;
    fur: boolean;
    cloth: boolean;
    clothRegions?: string[];
    furRegions?: string[];
    furShells?: number;
    moveSpeed?: number;
  };
  camera?: { mode: string; fovDeg: number; distanceM?: number; shoulderOffsetM?: number };
  controls?: { move: string; look: string; zoom: string; scheme?: string };
  movementModes?: MovementMode[];
  abilities?: AbilityDef[];
  overlay?: { toggle: string };
  dynamicLights?: DynamicLightDef[];
};

export const DEFAULT_MANIFEST: GameManifest = {
  title: "Snow Field",
  targetFps: 90,
  renderer: "webgpu",
};

export async function loadManifest(url = "/manifest.json"): Promise<GameManifest> {
  const res = await fetch(url);
  if (!res.ok) {
    throw new Error(`Failed to load manifest: ${res.status}`);
  }
  return (await res.json()) as GameManifest;
}

export function mergeManifest(base: GameManifest, patch: Partial<GameManifest>): GameManifest {
  return { ...base, ...patch };
}
