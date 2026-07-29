import {
  Camera,
  DefaultRenderingPipeline,
  ImageProcessingConfiguration,
  Scene,
} from "@babylonjs/core";
import type { GameManifest } from "../manifest.ts";

export type PostStageName =
  | "taa"
  | "ssao"
  | "ssr"
  | "dof"
  | "bloom"
  | "tonemap"
  | "grain"
  | "sharpen";

export type PostChainHandle = {
  pipeline: DefaultRenderingPipeline | null;
  ssao: null;
  toggles: Record<PostStageName, boolean>;
  labels: Record<PostStageName, string>;
  setEnabled: (stage: PostStageName, on: boolean) => void;
  dispose: () => void;
};

const STAGE_DEFAULTS: Record<PostStageName, boolean> = {
  taa: true,
  ssao: false,
  ssr: false,
  dof: false,
  bloom: true,
  tonemap: true,
  grain: false,
  sharpen: true,
};

const STAGE_LABELS: Record<PostStageName, string> = {
  taa: "fxaa",
  ssao: "ssao (off)",
  ssr: "ssr (off)",
  dof: "dof (off)",
  bloom: "bloom",
  tonemap: "aces",
  grain: "grain",
  sharpen: "sharpen",
};

export function createPostChain(
  scene: Scene,
  camera: Camera,
  manifest: GameManifest,
  opts?: { bypass?: boolean },
): PostChainHandle {
  const toggles = { ...STAGE_DEFAULTS };
  if (manifest.post) {
    for (let i = 0; i < manifest.post.length; i++) {
      const p = manifest.post[i]!;
      const name = p.name as PostStageName;
      if (name in toggles) toggles[name] = p.enabled;
    }
  }
  // Hard-disable stages that need a geometry buffer we do not own yet.
  toggles.ssao = false;
  toggles.dof = false;
  toggles.ssr = false;

  if (opts?.bypass === true) {
    const setEnabled = (stage: PostStageName, on: boolean): void => {
      toggles[stage] = on;
    };
    return {
      pipeline: null,
      ssao: null,
      toggles,
      labels: { ...STAGE_LABELS },
      setEnabled,
      dispose: () => undefined,
    };
  }

  const pipeline = new DefaultRenderingPipeline("post", true, scene, [camera]);
  pipeline.samples = 1;
  pipeline.fxaaEnabled = toggles.taa;
  pipeline.sharpenEnabled = toggles.sharpen;
  pipeline.bloomEnabled = toggles.bloom;
  pipeline.bloomThreshold = 0.92;
  pipeline.bloomWeight = 0.22;
  pipeline.bloomKernel = 40;
  pipeline.imageProcessingEnabled = toggles.tonemap;
  pipeline.imageProcessing.toneMappingEnabled = true;
  pipeline.imageProcessing.toneMappingType = ImageProcessingConfiguration.TONEMAPPING_ACES;
  pipeline.imageProcessing.exposure = 0.95;
  pipeline.imageProcessing.contrast = 1.12;
  pipeline.grainEnabled = toggles.grain;
  pipeline.depthOfFieldEnabled = false;

  const setEnabled = (stage: PostStageName, on: boolean): void => {
    if (stage === "ssao" || stage === "dof" || stage === "ssr") {
      toggles[stage] = false;
      return;
    }
    toggles[stage] = on;
    switch (stage) {
      case "taa":
        pipeline.fxaaEnabled = on;
        break;
      case "bloom":
        pipeline.bloomEnabled = on;
        break;
      case "tonemap":
        pipeline.imageProcessingEnabled = on;
        break;
      case "grain":
        pipeline.grainEnabled = on;
        break;
      case "sharpen":
        pipeline.sharpenEnabled = on;
        break;
    }
  };

  return {
    pipeline,
    ssao: null,
    toggles,
    labels: { ...STAGE_LABELS },
    setEnabled,
    dispose: () => pipeline.dispose(),
  };
}
