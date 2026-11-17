**DraimHorizon 0.41.0 — Sunrise groundwork**

Target: official Horizon Nightly source `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0` (unchanged). The Horizon bundle and runtime behavior are unchanged.

- Experimental groundwork for also supporting the Sunrise client (`sunrise-org/sunrise`): a Sunrise Nightly pin and mixin-tool pin in `compatibility.json`, a staleness check in `tools/compatibility.py`, and `tools/sunrise.py` (`resolve`, `generate`) with `platforms/sunrise/`. Sunrise itself is not supported yet.
