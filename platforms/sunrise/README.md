# Sunrise platform (experimental)

DraimHorizon is written once against Horizon's source (`mixins/draimhorizon`). Sunrise is a fork with
renamed crates, a different module layout and its own native hook model, so the Sunrise variant is
derived by `tools/sunrise.py` from the shared sources plus the files here:

- `platform.toml` - injections dropped or retargeted for Sunrise and relocated module destinations.
- `overlay/` - Sunrise-specific source files that replace or extend the shared ones.

Status: groundwork only. The Sunrise bundle is not yet compiled against the pinned Sunrise Nightly,
and the mod must not be advertised as supported there until it is.
