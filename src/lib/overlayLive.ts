export type OverlayBloubLiveInput = {
  shown: boolean;
  collapsed: boolean;
  peeked: boolean;
  hovered: boolean;
};

/** 贴边圆点且指针不在上面时停动画；展开、悬停预览、长条都继续演。
 *  测试表与 `src-tauri/src/desktop.rs` 的 `overlay_live_tests` 保持一致。
 */
export function overlayBloubLive({
  shown,
  collapsed,
  peeked,
  hovered,
}: OverlayBloubLiveInput): boolean {
  if (!shown) return false;
  if (!collapsed) return true;
  return peeked || hovered;
}

export function bloubLoopActive(live: boolean, reducedMotion: boolean): boolean {
  return live && !reducedMotion;
}
