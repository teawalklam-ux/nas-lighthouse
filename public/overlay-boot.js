(function () {
  try {
    var meta = window.__TAURI_INTERNALS__ && window.__TAURI_INTERNALS__.metadata;
    var label = meta && meta.currentWindow && meta.currentWindow.label;
    var overlay =
      label === "overlay" ||
      /[?&]window=overlay(?:&|$)/.test(location.search) ||
      location.hash === "#overlay";
    if (overlay) {
      document.documentElement.classList.add("overlay-root");
      document.documentElement.style.setProperty("background", "transparent", "important");
      document.documentElement.style.colorScheme = "only light";
    }
  } catch (e) {}
})();
