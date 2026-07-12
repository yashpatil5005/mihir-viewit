export { default as Viewer } from './Viewer.svelte';
export { default as TextViewer } from './TextViewer.svelte';
export { default as UnsupportedViewer } from './UnsupportedViewer.svelte';
export { default as PlaceholderViewer } from './PlaceholderViewer.svelte';

// Re-export viewers for explicit-import style (apps may use either path).
export type { DocumentKind } from '@viewit/platform';
