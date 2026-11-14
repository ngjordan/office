# The Workspace suite, served as static files.
#
# The build is a wasm32 bundle of the whole suite: one module, one canvas. The
# image therefore holds the built site and nothing else, so it is small and the
# runtime is nginx's, which is what a static site needs.
#
# The wasm is built here rather than copied in. The site is 160 MB and it is
# not in the repository: `dist/` is gitignored, because a build artefact does
# not belong in git. So the image builds it, which needs the Rust toolchain
# and the wasm target, and then throws both away: the final image holds only
# the site.
#
# The build is slow. Fat LTO over nine app engines takes about fifteen
# minutes and needs a lot of memory, so the layer is cached and only the
# parts that change are rebuilt.
#
#   docker build -f packaging/web/Dockerfile -t workspace-web .
#   docker run -p 8080:80 workspace-web

FROM rust:1.90-bookworm AS build

ARG APP_DIR=apps/iridium-office-web

# The fonts and the translations are build inputs, not dependencies. Without
# them the app builds and runs, but it has no Japanese face and the UI text
# is untranslated.
ENV CRAFT_FONTS_DIR=/tmp/craft-fonts
ENV CRAFT_I18N_DIR=/tmp/craft-i18n

RUN rustup target add wasm32-unknown-unknown \
    && cargo install trunk --locked --version 0.21.14 \
    && apt-get update \
    && apt-get install -y --no-install-recommends git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src

# The whole tree. The suite links the nine apps through the vendor tree, and
# those apps live in their own repositories: the CI clones them, and so does
# this image, because a path dependency outside the build context cannot be
# copied in.
COPY . .

# The apps are cloned beside the suite, and the vendor tree points at them.
# The clone list is the one in packaging/setup-vendor.sh.
RUN for pair in sheets:gc-gridcraft docs:wc-wordcraft cad:cc-cadcraft \
             decks:dc-deckcraft pdf:pdfcraft mail:mc-mailcraft \
             photos:pc-photocraft film:fc-filmcraft light:lc-lightcraft; do \
        name="${pair%%:*}"; dir="${pair##*:}"; \
        git clone --depth 1 "https://github.com/ngjordan/$name.git" "/$dir"; \
    done \
    && ./packaging/setup-vendor.sh / \
    && git clone --depth 1 https://github.com/ngjordan/craft-fonts.git "$CRAFT_FONTS_DIR" \
    && git clone --depth 1 https://github.com/ngjordan/craft-i18n.git "$CRAFT_I18N_DIR" \
    && rm -rf /src/office-vendor \
    && ls -la /src/office-vendor

ENV CARGO_BUILD_JOBS=4

RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cd ${APP_DIR} \
    && trunk build --release \
    && test -f dist/web/index.html \
    && ls -la dist/web/

FROM nginx:1.27-alpine

# The site is a single page plus its wasm. The wasm is large and immutable, so
# it is served with a long cache lifetime; the html and the loader are not,
# or a new build would never be picked up.
COPY --from=build /src/${APP_DIR}/dist/web/ /usr/share/nginx/html/

COPY packaging/web/nginx.conf /etc/nginx/conf.d/default.conf

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -q -O /dev/null http://127.0.0.1/ || exit 1

EXPOSE 80
