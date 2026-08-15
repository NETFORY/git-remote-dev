# git-remote-dev

**Git Remote Helper for the `dev://` protocol of the NETFORY network.**
Provides familiar `git clone / push / pull dev://user/repo` commands — no
relearning required, with native support in any IDE (VS Code, WebStorm,
GitKraken), because the project is managed by your standard `git` binary.

## Requirements

- `git` ≥ 2.30 in `PATH` (the bridge uses `git bundle` under the hood);
- a running **NETFORY client** with an unlocked wallet —
  the bridge communicates with it via loopback IPC (takes port/token from
  the discovery file, see below);
- Rust toolchain — only for building.

## Building and Installing

```bash
cargo build --release
```

Place the binary in a directory from `PATH` **strictly named** `git-remote-dev`
(git looks for the helper by URL scheme: `dev://` → `git-remote-dev`):

```bash
# Linux / macOS
install -m 0755 target/release/git-remote-dev ~/.local/bin/git-remote-dev
# make sure ~/.local/bin is in PATH:  echo $PATH

# Windows (PowerShell)
copy target\release\git-remote-dev.exe C:\Users\<you>\bin\git-remote-dev.exe
# directory C:\Users\<you>\bin must be in %PATH%
```

Verify installation:

```bash
git clone dev://<your-username>/<your-repository>
```

## Configuration

No configuration needed. The bridge automatically finds the running client via
the discovery file, which the client writes on startup:

| OS | Path |
|---|---|
| Linux / macOS | `~/.config/smartnet/devhub-ipc.json` |
| Windows | `%APPDATA%\smartnet\devhub-ipc.json` |

Contents: `{"port": <loopback-port>, "token": "<random-token>"}`.
The token protects IPC from other local processes; the server listens **only**
on `127.0.0.1`.

## Usage

```bash
git clone dev://technolog/smart-swarm     # clone from P2P network
cd smart-swarm
git add . && git commit -m "feat: …"      # regular local work
git push origin main                      # publish: signature + DHT announcement
```

- `git push` to a repository name that doesn't exist yet **auto-creates** it
  in your account (like on GitHub).
- Each push increments the signed `seq` counter of your index —
  anti-rollback: the network will never accept an older version.
- Cloning someone else's repositories works via swarm downloading
  bundles by signed `bundle_hash` (seeders are found through
  Mainline DHT + UDP identity beacons).

## Common Errors

| Message                                           | Cause / Fix |
|---------------------------------------------------|---|
| `NETFORY client not running (no discovery file…)` | Open the NETFORY app and try again. |
| `Wallet locked — unlock with PIN in the client`   | Unlock the wallet in the client (push is signed with your key). |
| `push is allowed only to your own repositories`   | The URL points to someone else's username. |
| `repository seeders are currently offline`        | No peer with the bundle is online — try again later. |
| `git not found in PATH`                           | Install git / add it to PATH. |

Detailed architecture description: `docs/09-DevHub-Git-Remote-Helper.md`.
