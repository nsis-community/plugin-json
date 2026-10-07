# JSON plug-in for NSIS

![License](https://img.shields.io/github/license/nsis-community/plugin-json?color=blue&style=for-the-badge)
![Release](https://img.shields.io/github/v/release/nsis-community/plugin-json?style=for-the-badge)
![CI](https://img.shields.io/github/actions/workflow/status/nsis-community/plugin-json/ci.yml?style=for-the-badge)

Read and write JSON and JSONC files from NSIS scripts, keeping comments, spacing and key order intact.

> [!NOTE]
> **Looking for the usage guide?** Commands, paths, recipes and caveats are in [Docs/JSON/README.md](Docs/JSON/README.md).

## Installation

Download the installer or archive from the [Releases page](https://github.com/nsis-community/plugin-json/releases).

If you downloaded the zip archive, extract it into your NSIS folder: it adds `JSON.dll` to `Plugins/<variant>/` and `JSON.nsh` to `Include/`.

The plug-in works as soon as the DLL is in place. For `${JsonForEach}` loops, add `!include "JSON.nsh"` to your script.

## Building

Needs [mise](https://mise.jdx.dev). `mise run checks` runs formatting, lints and tests, and `mise run build` writes both variants to `dist/Plugins/`. `mise run smoke` builds the smoke installer and runs it under Wine.

## License

[MIT](LICENSE)
