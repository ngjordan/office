# The Workspace suite, served as static files.
#
# The build is a wasm32 bundle of the whole suite: one module, one canvas. The
# image therefore holds the built site and nothing else, so it is small and the
# runtime is nginx's, which is what a static site needs.
#
# The build itself is not in this file. It takes about twenty minutes with fat
# LTO over nine app engines, and it needs the whole Rust toolchain plus the
# wasm target, so it belongs in the CI workflow that publishes the site. This
# image serves what that workflow produced.
#
#   docker build -f packaging/web/Dockerfile -t workspace-web .
#   docker run -p 8080:80 workspace-web

FROM nginx:1.27-alpine

# The site is a single page plus its wasm. The wasm is large and immutable, so
# it is served with a long cache lifetime; the html and the loader are not,
# or a new build would never be picked up.
COPY dist/web/ /usr/share/nginx/html/

COPY packaging/web/nginx.conf /etc/nginx/conf.d/default.conf

# The suite needs a large WebAssembly memory: nine app engines, plus the
# browser's own copies of the module.
#   4 GB of wasm memory is the ceiling Chrome enforces for a 32-bit wasm
#   module, so the suite asks for what it can get and fails loudly rather
#   than silently.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -q -O /dev/null http://127.0.0.1/ || exit 1

EXPOSE 80
