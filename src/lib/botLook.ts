import { clamp } from "../vendor/bloub/math";
import type { Look } from "../vendor/bloub/engine";

/** 跟随时的最大转头角度。比待机自带的轻晃更大，又不会把眼睛转到球体背面。 */
const YAW_MAX = 22;
const PITCH_MAX = 16;
/** 光标在正前方时略抬眼，看起来是在留意，而不是盯着地面。 */
const PITCH = 8;

export type ScreenPointer = {
  cursorX: number;
  cursorY: number;
  originX: number;
  originY: number;
  scale: number;
  spanX: number;
  spanY: number;
};

export function lookAtPointer(nx: number, ny: number): Look {
  return {
    yaw: nx * YAW_MAX,
    pitch: PITCH - ny * PITCH_MAX,
    mix: 1,
    spin: 0,
    wander: 0.28,
  };
}

export function pointerFromScreen(
  pointer: ScreenPointer,
  box: { left: number; top: number; width: number; height: number },
): { nx: number; ny: number } | null {
  if (box.width < 1 || box.height < 1) return null;
  const cx = pointer.originX + (box.left + box.width / 2) * pointer.scale;
  const cy = pointer.originY + (box.top + box.height / 2) * pointer.scale;
  const halfX = Math.max(1, pointer.spanX / 2);
  const halfY = Math.max(1, pointer.spanY / 2);
  const nx = clamp((pointer.cursorX - cx) / halfX, -1, 1);
  const ny = clamp((pointer.cursorY - cy) / halfY, -1, 1);
  if (!Number.isFinite(nx + ny)) return null;
  return { nx, ny };
}
