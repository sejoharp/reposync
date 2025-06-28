<!-- TOC -->
- [Usage](#usage)
- [Installation](#installation)
  - [install release](#install-release)
  - [install from source](#install-from-source)
  - [set config](#set-config)
- [Development](#development)
  - [create a release](#create-a-release)
- [benchmarks](#benchmarks)
  - [single threaded](#single-threaded)
  - [tokio spawn](#tokio-spawn)
  - [tokio spawn\_blocking](#tokio-spawn_blocking)
<!-- TOC -->


A tool to keep local repos up to date and clone new repos from a given team.

# Usage

```shell
reposync
```

The output will look like this:

```
Pulled 56 repos.
Cloned 0 repos.
```

# Installation

## install release
```bash
bash -c "$(curl -fsSL https://raw.githubusercontent.com/sejoharp/reposync/refs/heads/main/install.sh)"
```

## install from source
```shell
# install rust
brew install rustup-init

# build and install reposync
make install
```

## set config

```bash
export GITHUB_TEAM_REPO_URL=https://api.github.com/organizations/[org-id]/team/[team-id]/repos
export REPO_ROOT_DIR=[dir/to/repo/root]
export GITHUB_TEAM_PREFIX=team_
```

# Development

## create a release
1. make a commit 
2. push it
3. github actions will create a release

# benchmarks
## single threaded
cargo run  3.21s user 4.07s system 6% cpu 1:51.83 total

## tokio spawn
cargo run  1.78s user 1.88s system 24% cpu 14.688 total
cargo run  1.71s user 1.73s system 30% cpu 11.267 total

## tokio spawn_blocking
cargo run  1.62s user 2.17s system 71% cpu 5.344 total
