**DraimHorizon 0.42.0 — Sunrise SMP brand adapter prototype**

The Horizon target remains official Nightly `9f42064c11efe7ef3e5ed2baad7ddc2a3b36eea0`.

A separate native Sunrise project in `platforms/sunrise/smp-compat` implements the policy to select
`horizon` as the initial client brand only for `playhorizon.dev` and its subdomains. It includes a
minimal upstream patch to expose the required function through Sunrise's documented native route.

**Not installable on the current official Sunrise SDK:** its function index does not expose that
replacement point. Maintainers must review the patch and publish a new SDK, then the prototype
must be repinned and built. The current pin is the patch baseline, not a claim of runtime support.

Policy tests and the SDK API entry-point type check are separate from launcher replacement and
live SMP entry, which are not verified. Full DraimHorizon porting, Horizon extension channels and
content matching remain separate work. See the adapter's README for exact prerequisites.
