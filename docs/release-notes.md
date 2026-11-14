**DraimHorizon 0.40.2 - chat and sidebar cache correction**

Target: official Horizon Nightly source `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0` (unchanged). Maintenance follow-up to the 0.40.1 review; appearance and behavior are intended to be identical.

- Chat row cache evicts least-recently-used rows one at a time instead of clearing everything at the count or size limit, so rows that are still painted survive a window resize that creates a new layout every frame. Widths are still exact; nothing is rounded or throttled.
- A card's text digest is computed once per query and shared by the full and row-cut lookups and inserts (previously up to four times). Stored inputs are still compared in full on every hit.
- Oversized layouts are rejected without evicting valid rows; colliding entries and Full/Cut variants stay independent when one is evicted.
- The interface shape cache now accounts replaced runs correctly (growth and shrinkage), and replacing a run at the run limit no longer flushes the others.

Checks: seven new regressions failed before the change and pass after; the full workspace suite passed 561 tests (one ignored), along with formatting and Clippy. The pinned native client builds in the optimized profile and the bundle validates. Isolated live runs (baseline 0.40.1 vs this build, same local world, EN and RU, GUI scales 2 and 3, scrolling, window sizes from 900x600 to 2560x1440 and a 60-step programmatic window resize, cut rows, negative-spacing fixture) rendered the inspected chat area identically except a 1-level animation delta on tab pills. Microbenchmarks: a resize sweep with 8 stable rows rebuilt them 12520 times before and 8 after on long messages (400.9 to 251.9 us/frame); warm lookups of short messages cost about 16 ns more, long ones about 330 ns less. Short-message resize workloads were also slightly slower (10.0 to 11.3 us/frame). These are isolated cache timings, not live frame-time measurements; no FPS improvement is claimed.
