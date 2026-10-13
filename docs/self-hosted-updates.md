# Self-hosted updates (optional, web deployments only)

For people who serve PhotoCraft's web build from their own server. **It is off by default** and has
nothing to do with the desktop app or with the release packages: a build without the feature makes
no update requests and shows nothing.

With it, a self-hosted site:

- shows every visitor a notice above the canvas when a newer PhotoCraft release exists
  ("A new version (0.6.1) is available. This site runs 0.4.0."),
- lets the administrator sign in from the notice and choose **Update now**,
- emails the administrator when a new release is first seen and when an update finishes.

**This is not the desktop update check.** The desktop app's "Help > Check for Updates" (proposed in
#2295, tracked with the wider update-button discussion in #2375) is a separate feature; this one
lives only in `apps/photocraft-web` and changes nothing on desktop.

The work is done on the server by `craft-updated`, a small companion process from
[craft-libs' `craft-update` crate](https://github.com/storytold/craft-libs/tree/main/crates/update).
It listens on a loopback address behind a reverse-proxied path, checks the administrator's
sign-in on every request, and on **Update now** fetches the newest release tag into its own clone,
builds it with its configured command and swaps the served folder atomically. The browser can't
choose what is built; it can only ask for the newest release the service found itself.

## Setup in short

```sh
# a clone for the updater, built with the feature
git clone https://github.com/storytold/photocraft.git /srv/photocraft/src
cd /srv/photocraft/src/apps/photocraft-web && trunk build --release --features self-hosted-update
mkdir -p /srv/photocraft/releases && cp -r /srv/photocraft/src/dist/web /srv/photocraft/releases/0.6.1-0
ln -sfn /srv/photocraft/releases/0.6.1-0 /srv/photocraft/live     # serve this link

# the companion service
cargo install --locked --git https://github.com/storytold/craft-libs craft-update --features server
craft-updated setup --config /srv/photocraft/craft-updated.toml  # admin, SMTP, build paths
craft-updated serve --config /srv/photocraft/craft-updated.toml
```

Proxy `craft-update/` next to the app to it (nginx):

```nginx
location /photocraft/ { alias /srv/photocraft/live/; add_header Cache-Control "no-cache"; }
location /photocraft/craft-update/ { proxy_pass http://127.0.0.1:8790/; client_max_body_size 4k; }
```

The defaults `craft-updated setup` offers for PhotoCraft are the build command
`trunk build --release --features self-hosted-update` in `apps/photocraft-web`, with output in
`dist/web`. The service account needs Rust, the `wasm32-unknown-unknown` target and `trunk`.
The full guide (systemd unit, Caddy, every setting, the security model and its limits) is in the
crate's README.

## In the code

`apps/photocraft-web/src/updates.rs` (compiled only with the feature) draws the notice in the web
shell before the app and passes its text through the interface catalog, so translations can be
added like any other label. `cargo xtask ci` runs clippy, the tests and the wasm build with the
feature on (`self-hosted-update` step).
