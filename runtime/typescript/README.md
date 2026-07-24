# Bun TypeScript Runtime Templates

Future compiler-shipped harness templates for SIL modules routed to the async
I/O target. SIL emits TypeScript into `{workdir}/.runtime/typescript/`, and the
supervisor executes it with Bun, not Node.

This directory is part of the compiler scaffold. User-program output is not
generated here.
