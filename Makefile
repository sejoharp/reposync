.DEFAULT_GOAL:= help

.PHONY:dependencies 
dependencies: ## downloads and installs dependencies
	cargo update

.PHONY:test 
test: ## executes tests
	cargo test

.PHONY:build
build: ## builds binary with debug infos
	cargo build

.PHONY:bump-version-minor
bump-version-minor: ## updates minor version in Cargo.toml
	scripts/bump-version.sh minor

.PHONY:bump-version-patch
bump-version-patch: ## updates patch version in Cargo.toml
	scripts/bump-version.sh patch

.PHONY:release
release: ## builds release binary
	cargo build --release

.PHONY: install
install: release ## builds and installs `reposync` binary into ~/bin directory
	cp target/release/reposync ~/bin/reposync

.PHONY: help	
help: ## shows help message
	@awk 'BEGIN {FS = ":.*##"; printf "\nUsage:\n  make \033[36m\033[0m\n"} /^[$$()% a-zA-Z_-]+:.*?##/ { printf "  \033[36m%-15s\033[0m %s\n", $$1, $$2 } /^##@/ { printf "\n\033[1m%s\033[0m\n", substr($$0, 5) } ' $(MAKEFILE_LIST)

.PHONY: tag-release
tag-release: ## tags the current release commit
	git tag "v$$(cargo metadata --no-deps --format-version=1 | jq -r '.packages[0].version')"

.PHONY: push-release
push-release: ## pushes current branch and release tags
	git push
	git push --tags

.PHONY: commit-version-files
commit-version-files:
	git add Cargo.toml Cargo.lock
	git commit -m "chore(release): v$$(cargo metadata --no-deps --format-version=1 | jq -r '.packages[0].version')"

.PHONY: create-minor-release
create-minor-release: ## bumps minor version, builds, commits, tags, and pushes
	$(MAKE) bump-version-minor
	$(MAKE) release
	$(MAKE) commit-version-files
	$(MAKE) tag-release
	$(MAKE) push-release

.PHONY: create-patch-release
create-patch-release: ## bumps patch version, builds, commits, tags, and pushes
	$(MAKE) bump-version-patch
	$(MAKE) release
	$(MAKE) commit-version-files
	$(MAKE) tag-release
	$(MAKE) push-release