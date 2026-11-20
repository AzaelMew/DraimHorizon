# DraimHorizon 0.42.4 for Horizon Nightly `0a875da`

DraimHorizon 0.42.2 targets `dd67b76`, so Horizon Launcher excludes it on the November 20 Nightly. The updated `draimhorizons-0.42.4.hmod` targets `0a875da91dbb0fdcb7ba8c42c11341aa229af51a`.

This Nightly's launcher generates four game modules that the old package supplied as shims. The updated package uses the generated modules to avoid duplicate definitions.

Rebuild native mods after installing the updated `.hmod` on Horizon Nightly `0a875da`. The bundle keeps the `draimhorizon` ID and continues to use the existing settings and data.

Horizon Launcher built the client with DraimHorizon 0.42.4 included. The package passed the SDK check for all 76 mixins.
