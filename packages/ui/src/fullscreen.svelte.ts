// Shared fullscreen state for the viewer chrome.
//
// The app header owns the toggle (alongside Open file / Log / Browse), but
// each document handler decides what its own full screen mode looks like by
// reading `fullscreenState.active` — hide meta bars, drop stage caps, request
// native element fullscreen for media, etc.

export const fullscreenState = $state({ active: false });

export function setFullscreen(active: boolean): void {
  fullscreenState.active = active;
}

export function toggleFullscreen(): void {
  fullscreenState.active = !fullscreenState.active;
}
