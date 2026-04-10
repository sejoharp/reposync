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

.PHONY:version-update
version-update: ## updates version in Cargo.toml
	scripts/bump-version.sh patch

.PHONY:version-update-minor
version-update-minor: ## updates minor version in Cargo.toml
	scripts/bump-version.sh minor

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
	git tag v$(shell cargo pkgid | cut -d# -f2 | cut -d: -f2)

.PHONY: push-release
push-release: ## pushes current branch and release tags
	git push
	git push --tags

.PHONY: create-release
create-release: ## bumps minor version, builds, commits, tags, and pushes
	$(MAKE) version-update-minor
	$(MAKE) release
	git add Cargo.toml
	git commit -m "chore(release): v$(shell cargo pkgid | cut -d# -f2 | cut -d: -f2)"
	$(MAKE) tag-release
	$(MAKE) push-release

