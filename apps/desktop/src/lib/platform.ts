// Host platform of the webview, known synchronously (before `app_info` lands)
// so layout and key handling never flicker.

export const IS_MAC: boolean =
  typeof navigator !== "undefined" && /Mac|iPhone|iPad|iPod/.test(navigator.userAgent);

/** Width the native traffic lights occupy at the top-left on macOS. */
export const MAC_TRAFFIC_LIGHTS_WIDTH = 78;
