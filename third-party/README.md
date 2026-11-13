# Vendored crates

Two trees live here because the suite needs both at once.

## `craft-pdf`

A copy of `craft-pdf` from craft-libs @ `6e03f65`, so it can point at
craft-libs' OWN hayro tree (`DecodeLimits` and `Cancellation`) rather than the
office's single patched tree, which pdfcraft's crates need in its own shape.
The office's `[patch]` section redirects the craft-libs git source here.

## `hayro-craftlib`

The hayro family from craft-libs, with its versions bumped so cargo cannot
unify it with pdfcraft's hayro (`office-vendor/pdf/vendor/`). The two families
coexist because they are the same upstream crate patched at different points;
merging them into one tree does not compile.
