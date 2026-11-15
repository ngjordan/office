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

# The workspace needs rustc 1.95 or newer (every app's rust-version says so),
# and the toolchain the suite is built with is 1.99.
FROM rust:1.99-bookworm AS build

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

# The sources are fetched here rather than copied in.
#
# The deployment sends only this Dockerfile: the build context is empty, so
# there is nothing to copy. That also keeps the build small: the workspace's
# own tree is 53 GB with its build cache, and none of it belongs in an image.
#
# The commit is an ARGUMENT, and that matters: a layer is cached by its
# command and the tree before it, so a fixed branch name would be cached for
# ever and every deployment would serve the build from whenever it was first
# made. The argument makes the layer change with the commit.
ARG COMMIT=HEAD
RUN git clone --depth 1 https://github.com/ngjordan/office.git /src \
    && cd /src \
    && git fetch --depth 1 origin "$COMMIT" \
    && git checkout FETCH_HEAD

# The apps are cloned beside the suite, and the vendor tree points at them.
# The clone list is the one in packaging/setup-vendor.sh, and the revision each
# app is cloned at comes from packaging/web/apps.lock. Pinning them matters:
# the image is built from a commit, and a build layer is cached by its
# command, so tracking each app's branch HEAD would serve whichever commit
# happened to be pushed and would not pick up an app's new work at all.
RUN cd /src \
    && set -a && . ./packaging/web/apps.lock && set +a \
    && for pair in sheets:gc-gridcraft docs:wc-wordcraft cad:cc-cadcraft \
             decks:dc-deckcraft pdf:pdfcraft mail:mc-mailcraft \
             photos:pc-photocraft film:fc-filmcraft light:lc-lightcraft; do \
        name="${pair%%:*}"; dir="${pair##*:}"; sha="$(printenv "$name")"; \
        mkdir -p "/$dir" \
        && git -C "/$dir" init -q \
        && git -C "/$dir" remote add origin "https://github.com/ngjordan/$name.git" \
        && git -C "/$dir" fetch -q --depth 1 origin "$sha" \
        && git -C "/$dir" checkout -q FETCH_HEAD; \
    done \
    && git clone --depth 1 https://github.com/ngjordan/craft-fonts.git "$CRAFT_FONTS_DIR" \
    && git clone --depth 1 https://github.com/ngjordan/craft-i18n.git "$CRAFT_I18N_DIR" \
    && ./packaging/setup-vendor.sh / \
    && ls -la /office-vendor

ENV CARGO_BUILD_JOBS=12

# The cache mounts that would speed this up need BuildKit, which the daemon
# here does not have, so the build is plain.
#
# Trunk writes to `../../dist/web` (see the crate's Trunk.toml), so the site
# lands in the workspace's own dist/, not under the app.
RUN cd ${APP_DIR} \
    && trunk build --release \
    && test -f /src/dist/web/index.html \
    && ls -la /src/dist/web/

FROM nginx:1.27-alpine

# The site is a single page plus its wasm. The wasm is large and immutable, so
# it is served with a long cache lifetime; the html and the loader are not,
# or a new build would never be picked up.
COPY --from=build /src/dist/web/ /usr/share/nginx/html/

# The nginx configuration comes out of the clone, for the same reason.
COPY --from=build /src/packaging/web/nginx.conf /etc/nginx/conf.d/default.conf

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -q -O /dev/null http://127.0.0.1/ || exit 1

EXPOSE 80
