# Sunrise server performance and TPS

Use for server behavior, load, scheduling, and TPS tasks. Discover the actual server
stack and target tick rate; do not assume a language, engine, or universal TPS target.

## Before changing code

Establish the symptom, expected behavior, workload, affected revision, relevant
configuration, hardware, and available traces. Read existing measurements before
proposing new profiling. Identify the suspected bottleneck and the smallest useful
change. Keep behavior/correctness requirements explicit alongside performance goals.

## Approved experiment

Propose baseline and candidate revisions, player/entity counts or equivalent load,
warmup, duration, repetitions, environment, metrics, and success criteria. Obtain
approval before profiling, launching a server, generating traffic, or spending
resources. Production execution requires explicit authorization for that environment.

Use comparable hardware, configuration, and workload. Record tick-time distribution
including p95/p99 when available, sustained TPS, CPU, memory/GC, allocation or queue
pressure, and errors as relevant to the suspected bottleneck. Prefer existing tools.
Do not add a telemetry stack without a demonstrated need and approval.

## Interpretation

Compare baseline and candidate under the same conditions. Record variability and
possible confounders; do not present one noisy run as proof. A capped TPS reading
alone may hide reduced headroom. Average TPS can hide long stalls. Validate behavior
and stability as well as throughput.

Commit the implementation before the experiment. Commit each identified, authorized
fix separately. Keep large traces out of Git; link an approved artifact or include a
small sanitized summary. If execution is unavailable, report the hypothesis and
pending experiment, without an invented percentage improvement.
