# ViewIt Privacy

ViewIt is an offline-first file viewer. Documents selected or shared into ViewIt are
parsed and rendered locally on the device. ViewIt does not upload document contents,
file names, previews, or extracted text to ViewIt servers.

## Network use

ViewIt uses the network only to:
- fetch the signed optional-plugin catalog;
- download a plugin the user chooses to install;
- fetch a custom plugin catalog URL the user explicitly adds.

Plugins are checksum-verified against an Ed25519-signed catalog. After installation,
plugins and document viewers work offline. A plugin can access only the document URI
passed to it and app-private/cache storage unless the user explicitly chooses a folder
or save destination through Android's system document picker.

## Data storage

- Viewer preferences, playback resume positions, installed-plugin metadata, and plugin
  files are stored locally in app/private storage.
- Materialized document copies are stored in the app cache and pruned automatically.
- Users can clear all ViewIt data from Android Settings.

## Permissions

ViewIt uses Android's Storage Access Framework and content URI grants instead of broad
storage permissions. It does not request contacts, location, microphone, camera, or
advertising identifiers for document viewing.

Security issues can be reported through the repository instructions in `SECURITY.md`.
