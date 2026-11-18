**DraimHorizon 0.41.6 — Fix Sunrise declared-internals validation**

Fixes the Sunrise Launcher failure `E_INTERNAL_MISSING` naming
`sunrise_client_lib::net::ingress::Producer` in 0.41.4/0.41.5.

The SMP adapter incorrectly declared five SDK `shape_only` layout entries as accessible internals.
The package now declares only the exposed `Session` type. The native helpers and brand behavior are
unchanged; SDK and package preflight now enforce the launcher's declaration eligibility rules.

No game-client, SDK or player-cache changes are needed. The separate cache-cleanup warning in the
reported log is not the declared-internal failure. Validation of this fix is separate from a complete
launcher build and live SMP login; neither is claimed by the package checks alone.
