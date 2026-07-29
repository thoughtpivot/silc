/**
 * Shadow cascade helpers.
 * Beauty terrain is VS-displaced; stock CSM still receives character/wake casters.
 * Depth programs that share clipmap placement land in a follow-on when Babylon
 * custom depth materials are wired for the static clipmap mesh.
 */
export const CASCADE_SPLITS_M = [26, 95, 330] as const;
export const CASCADE_COUNT = 3;
export const CASCADE_RES = 2048;
