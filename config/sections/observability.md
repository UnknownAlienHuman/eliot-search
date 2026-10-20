# Observability settings disposition

`[observability]` is not registered by the current daemon, and `search-eval` is not a runtime logging owner.
The old rotation/content/privileged-debug table is not supported configuration.

Add only fields consumed by an actual logger/status implementation, for example a closed log level and finite
rotation limits. Ordinary diagnostics never contain credentials, source/query bodies or private absolute
paths. A privileged content-debug mode is absent until a concrete authenticated consumer and bounded lifetime
exist.

No section descriptor, binding-auth receipt, configuration fingerprint or generic live-reload platform.
