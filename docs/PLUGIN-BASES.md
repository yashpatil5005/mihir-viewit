# Legacy Plugin Bases

`base` is current manifest/UI metadata with values such as `view`, `play`, `edit`, and `tool`. It groups whole user experiences, but it is not a sufficient runtime contract and does not grant authority.

Current shipped examples include:

- `view`: Office, archive, font, iWork, and PPTX viewing extensions;
- `play`: Player Base;
- `edit`: Editor Base;
- tool-like behavior: archive extraction and media transcoding.

Runtime code also uses format lists, runtime type, Java interface casts, JavaScript export conventions, user preferences, and some canonical plugin IDs. This overlap is being replaced by nominal versioned service and provider descriptors.

Target examples include:

- `viewit.document.parse`
- `viewit.document.render`
- `viewit.document.edit`
- `viewit.archive.list`
- `viewit.archive.read-entry`
- `viewit.archive.extract`
- `viewit.media.play`
- `viewit.media.transcode`

`base` may remain as presentation/category metadata during migration. New runtime behavior must not depend on it as the provider interface.

See [`PLUGIN-ARCHITECTURE.md`](PLUGIN-ARCHITECTURE.md).
